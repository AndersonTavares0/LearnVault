import { tr } from './i18n';
import type { CatalogNode, FileEntry } from './transport';

export type FileCounts = { videos: number; materials: number };
export type CatalogCounts = FileCounts & { modules: number; lessons: number; missingLessons: number };

/**
 * Arquivo de mídia que a última varredura ainda encontra no disco.
 *
 * A comparação é tolerante (espaços e caixa) e um campo `availability` ausente vale como
 * disponível — a mesma regra tolerante já usada pelo roteador (`findLesson`) e pela lista
 * de arquivos soltos: um arquivo sem o campo nunca é tratado como ausente por omissão.
 */
function isFileAvailable(file: FileEntry): boolean {
	return file.availability?.trim().toLowerCase() !== 'missing';
}

/**
 * Aula sem nenhuma mídia reproduzível.
 *
 * Uma aula é ausente quando tem pelo menos um arquivo de mídia (`kind === 'media'`) e
 * nenhum deles está disponível. Só a mídia decide: legendas e materiais não reabrem uma
 * aula cuja mídia desapareceu, porque é a mídia que a linha abre no player — uma aula com
 * a mídia ausente e a legenda presente continua ausente para quem vai estudar.
 *
 * Uma aula sem nenhum arquivo de mídia (só material, ou vazia) nunca é ausente: não há
 * mídia para faltar.
 */
export function isLessonMissing(node: CatalogNode): boolean {
	const media = [...node.files, ...node.looseFiles].filter((file) => file.kind === 'media');
	return media.length > 0 && media.every((file) => !isFileAvailable(file));
}

/**
 * Arquivos do próprio nó, sem descer nos filhos.
 *
 * Legendas entram em `materials` de propósito: para quem estuda, elas são um recurso da
 * aula, não uma aula. É a contagem que descreve o que a linha de aula abre no player —
 * uma aula com subpastas de bônus conta os arquivos das subpastas em cada linha filha,
 * não na linha pai.
 */
export function countCatalogFiles(node: CatalogNode): FileCounts {
	const counts: FileCounts = { videos: 0, materials: 0 };
	for (const file of [...node.files, ...node.looseFiles]) {
		if (file.kind === 'media') counts.videos += 1;
		else if (file.kind === 'subtitle' || file.kind === 'material') counts.materials += 1;
	}
	return counts;
}

/**
 * Mídias da subárvore que o último rescan marcou como ausentes.
 *
 * A contagem de vídeos inclui registros ausentes de propósito (o catálogo não os apaga),
 * mas a ficha precisa dizer quantos realmente abrem — do contrário "2 vídeos" ao lado de
 * "1 aula" parece um erro. As aulas já usam a mesma distinção.
 */
export function countMissingVideos(node: CatalogNode): number {
	let missing = 0;
	function walk(current: CatalogNode): void {
		for (const file of [...current.files, ...current.looseFiles]) {
			if (file.kind === 'media' && !isFileAvailable(file)) missing += 1;
		}
		for (const child of current.children) walk(child);
	}
	walk(node);
	return missing;
}

/**
 * Contagens de um nó e de toda a sua subárvore.
 *
 * `modules` conta nós por papel; `videos` e `materials` somam os arquivos do nó e dos
 * descendentes. O próprio nó entra na contagem quando é uma aula, para que a meta de um
 * curso descreva a subárvore inteira.
 *
 * As aulas são contadas em duas somas: `lessons` traz as que têm mídia disponível e
 * `missingLessons` as que a varredura marcou como ausentes. A aula ausente continua
 * catalogada, listada e contada — só não se mistura com as que o aluno consegue abrir,
 * e por isso não infla a contagem de disponíveis.
 */
export function countCatalogNode(node: CatalogNode): CatalogCounts {
	const counts: CatalogCounts = { modules: 0, lessons: 0, missingLessons: 0, videos: 0, materials: 0 };
	function walk(current: CatalogNode): void {
		if (current.role === 'module') counts.modules += 1;
		if (current.role === 'lesson') {
			if (isLessonMissing(current)) counts.missingLessons += 1;
			else counts.lessons += 1;
		}
		const files = countCatalogFiles(current);
		counts.videos += files.videos;
		counts.materials += files.materials;
		for (const child of current.children) walk(child);
	}
	walk(node);
	return counts;
}

/**
 * Contagem de aulas de uma subárvore: as disponíveis e, quando houver, as ausentes.
 *
 * `4 aulas`, `4 aulas · 4 ausentes`, `1 aula · 3 ausentes`, `0 aulas · 4 ausentes`. A
 * contagem de disponíveis aparece sempre para que o total do catálogo continue legível;
 * a de ausentes só aparece quando existe, para não poluir um catálogo íntegro.
 *
 * Uma pasta-aula pode conter vários arquivos de mídia distintos (o modelo de catálogo
 * trata qualquer pasta-folha com arquivos como uma única aula, documentado em
 * `role_of` no núcleo Rust). Quando isso acontece, `videos` passa de `lessons +
 * missingLessons`, e a contagem de vídeos aparece ao lado para que "12 aulas" não pareça
 * baixo demais diante de um curso com 20 vídeos empacotados nessas 12 pastas.
 */
export function formatLessonCounts(counts: CatalogCounts): string {
	return joinParts([
		formatCount(counts.lessons, tr('lesson', 'aula'), tr('lessons', 'aulas')),
		counts.missingLessons > 0 ? formatCount(counts.missingLessons, tr('missing', 'ausente'), tr('missing', 'ausentes')) : '',
		counts.videos > counts.lessons + counts.missingLessons
			? formatCount(counts.videos, tr('video', 'vídeo'), tr('videos', 'vídeos'))
			: ''
	]);
}

export function formatCount(value: number, singular: string, plural: string): string {
	return `${value} ${value === 1 ? singular : plural}`;
}

export type CourseProgressStatus = 'all' | 'in_progress' | 'not_started' | 'completed';

/**
 * Determina o status de estudo de um curso com base nas aulas concluídas e em andamento.
 */
export function getCourseProgressStatus(
	courseNode: CatalogNode,
	completedMediaIds: Set<string>,
	inProgressMediaIds?: Set<string>
): 'completed' | 'in_progress' | 'not_started' {
	let totalLessons = 0;
	let completedLessons = 0;
	let hasInProgress = false;

	function walk(current: CatalogNode): void {
		if (current.role === 'lesson') {
			if (!isLessonMissing(current)) {
				const mediaFiles = [...current.files, ...current.looseFiles].filter((f) => f.kind === 'media' && f.mediaId);
				if (mediaFiles.length > 0) {
					totalLessons += 1;
					const isCompleted = mediaFiles.some((f) => f.mediaId && completedMediaIds.has(f.mediaId));
					if (isCompleted) {
						completedLessons += 1;
					} else if (inProgressMediaIds && mediaFiles.some((f) => f.mediaId && inProgressMediaIds.has(f.mediaId))) {
						hasInProgress = true;
					}
				}
			}
		}
		for (const child of current.children) walk(child);
	}

	walk(courseNode);

	if (totalLessons > 0 && completedLessons === totalLessons) return 'completed';
	if (completedLessons > 0 || hasInProgress) return 'in_progress';
	return 'not_started';
}

/** Completed and total playable lessons of a course; a lesson counts once even with several videos. */
export function courseProgress(courseNode: CatalogNode, completedMediaIds: Set<string>): { done: number; total: number } {
	let done = 0;
	let total = 0;
	const walk = (current: CatalogNode): void => {
		if (current.role === 'lesson' && !isLessonMissing(current)) {
			const ids = [...current.files, ...current.looseFiles].flatMap((file) => (file.kind === 'media' && file.mediaId ? [file.mediaId] : []));
			if (ids.length > 0) {
				total += 1;
				if (ids.some((id) => completedMediaIds.has(id))) done += 1;
			}
		}
		current.children.forEach(walk);
	};
	walk(courseNode);
	return { done, total };
}

function joinParts(parts: string[]): string {
	return parts.filter(Boolean).join(' · ');
}

/** Course metadata: modules, lessons (available and missing), and materials; zero counts are omitted. */
export function formatCourseMeta(counts: CatalogCounts): string {
	return joinParts([
		counts.modules > 0 ? formatCount(counts.modules, tr('module', 'módulo'), tr('modules', 'módulos')) : '',
		counts.lessons > 0 || counts.missingLessons > 0 ? formatLessonCounts(counts) : '',
		counts.materials > 0 ? formatCount(counts.materials, tr('material', 'material'), tr('materials', 'materiais')) : ''
	]);
}

/** Short metadata for a course card: the essentials for choosing what to study. */
export function formatCourseCardMeta(counts: CatalogCounts): string {
	return joinParts([
		counts.modules > 0 ? formatCount(counts.modules, tr('module', 'módulo'), tr('modules', 'módulos')) : '',
		counts.lessons > 0 || counts.missingLessons > 0 ? formatLessonCounts(counts) : ''
	]);
}

/** Lesson row metadata: videos and materials in the node itself, omitting zero counts. */
export function formatLessonMeta(counts: FileCounts): string {
	return joinParts([
		counts.videos > 0 ? formatCount(counts.videos, tr('video', 'vídeo'), tr('videos', 'vídeos')) : '',
		counts.materials > 0 ? formatCount(counts.materials, tr('material', 'material'), tr('materials', 'materiais')) : ''
	]);
}

/**
 * Materiais (não mídia) de um curso inteiro, sem repetir o mesmo caminho.
 *
 * A ficha do curso mostra tudo o que acompanha as aulas, inclusive o que está dentro de
 * módulos, em uma lista única. Legendas e arquivos de apoio entram; os vídeos ficam de
 * fora porque já são as aulas.
 */
export function collectMaterials(node: CatalogNode): FileEntry[] {
	const byPath = new Map<string, FileEntry>();
	function walk(current: CatalogNode): void {
		for (const file of [...current.files, ...current.looseFiles]) {
			if (file.kind !== 'media' && !byPath.has(file.path)) byPath.set(file.path, file);
		}
		for (const child of current.children) walk(child);
	}
	walk(node);
	return [...byPath.values()];
}

/** Contexto de origem de um item recente: o curso e, quando houver, o módulo. */
export function formatLocationContext(courseTitle: string, moduleTitle: string | null): string {
	return joinParts([courseTitle, moduleTitle ?? '']);
}

/**
 * Remove prefixos numéricos e separadores de pastas de categoria para exibição legível.
 *
 * Ex: "01 - Programacao e Desenvolvimento" -> "Programacao e Desenvolvimento"
 *     "02. Design de Interfaces" -> "Design de Interfaces"
 */
export function formatCategoryName(folder: string): string {
	const trimmed = folder.trim();
	const cleaned = trimmed.replace(/^\d+[\s._-]+/, '').trim();
	return cleaned || trimmed;
}

