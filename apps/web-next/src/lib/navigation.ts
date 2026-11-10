import type { CatalogNode, FileEntry } from './transport';

export type LessonRef = { title: string; path: string };
export type LessonSection = { title: string | null; lessons: LessonRef[] };

type IndexedLesson = { ref: LessonRef; ancestors: CatalogNode[] };

/**
 * Índice das aulas na ordem canônica do catálogo.
 *
 * A ordem é a da própria árvore (busca em profundidade), que é a ordem que o catálogo
 * Rust já entrega: categoria, curso, módulo e aula, com os módulos antes das aulas
 * soltas do curso. Percorrer a árvore em vez das categorias projetadas mantém uma única
 * fonte de verdade — a árvore canônica — e evita duplicar aulas quando as duas
 * projeções existem. Cada aula guarda a cadeia de ancestrais para que o agrupamento
 * saiba a qual curso ou módulo ela pertence sem uma segunda travessia.
 */
function indexLessons(nodes: CatalogNode[]): IndexedLesson[] {
	const indexed: IndexedLesson[] = [];
	function walk(list: CatalogNode[], ancestors: CatalogNode[]): void {
		if (!Array.isArray(list)) return;
		for (const node of list) {
			if (!node) continue;
			if (node.role === 'lesson') indexed.push({ ref: { title: node.title, path: node.path }, ancestors });
			if (Array.isArray(node.children) && node.children.length > 0) walk(node.children, [...ancestors, node]);
		}
	}
	walk(nodes, []);
	return indexed;
}

/** Todas as aulas do catálogo, na ordem canônica, sem repetir nenhuma. */
export function lessonSequence(nodes: CatalogNode[]): LessonRef[] {
	return indexLessons(nodes).map((entry) => entry.ref);
}

/**
 * Aula anterior e seguinte na ordem canônica do catálogo.
 *
 * Os limites devolvem `null` em vez de dar a volta, para que a interface esconda o
 * controle em vez de prometer uma navegação que não existe. Um caminho ausente ou
 * desconhecido (por exemplo, depois de uma reindexação que removeu a aula) também
 * devolve os dois lados nulos: o player não inventa vizinhos para uma aula que sumiu.
 */
export function neighboringLessons(nodes: CatalogNode[], lessonPath: string | null): { previous: LessonRef | null; next: LessonRef | null } {
	if (lessonPath === null) return { previous: null, next: null };
	const sequence = lessonSequence(nodes);
	const position = sequence.findIndex((ref) => ref.path === lessonPath);
	if (position < 0) return { previous: null, next: null };
	return { previous: sequence[position - 1] ?? null, next: sequence[position + 1] ?? null };
}

/**
 * Contêiner de seção: o ancestral mais próximo que agrupa aulas — módulo, curso ou
 * categoria.
 *
 * A seção é o agrupamento visível mais próximo da aula: `categoria/curso/módulo/aula`
 * fica sob o módulo; sem módulo, sob o curso; sem curso, sob a categoria. Aula sem
 * contêiner (árvore atípica, sem categorias) cai numa seção sem título, para o índice
 * continuar útil em vez de vazio.
 */
function sectionContainer(ancestors: CatalogNode[]): CatalogNode | null {
	return [...ancestors].reverse().find((node) => node.role === 'course' || node.role === 'module' || node.role === 'category') ?? null;
}

/**
 * Seções do índice de aulas: todas as aulas do catálogo agrupadas pelo contêiner mais
 * próximo, na ordem canônica.
 *
 * Contêineres são identificados pelo nó da árvore (caminho completo): dois cursos com o
 * mesmo título em caminhos diferentes não se fundem. A ordem das seções é a da primeira
 * aula de cada uma, então o índice nunca fica fora da ordem do catálogo.
 */
export function lessonSections(nodes: CatalogNode[]): LessonSection[] {
	const sections: LessonSection[] = [];
	const byContainer = new Map<CatalogNode | null, LessonSection>();
	for (const entry of indexLessons(nodes)) {
		const container = sectionContainer(entry.ancestors);
		let section = byContainer.get(container);
		if (!section) {
			section = { title: container?.title ?? null, lessons: [] };
			byContainer.set(container, section);
			sections.push(section);
		}
		section.lessons.push(entry.ref);
	}
	return sections;
}

// Mídia tocável: um arquivo de mídia que a última varredura não marcou como ausente.
function playableMedia(file: FileEntry): boolean {
	return file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing';
}

// Aula reproduzível: pelo menos uma mídia tocável entre os arquivos da própria aula.
function playableLesson(node: CatalogNode): boolean {
	return [...node.files, ...node.looseFiles].some(playableMedia);
}

/**
 * Primeira aula disponível na ordem canônica.
 *
 * Aulas têm preferência sobre mídia solta, porque é a aula que a lista lateral destaca.
 * Sem nenhuma aula reproduzível, o primeiro arquivo solto tocável também serve: um curso
 * só com mídia solta continua abrindo alguma coisa em vez de uma tela vazia.
 */
export function firstAvailableLessonPath(nodes: CatalogNode[]): string | null {
	function findLesson(list: CatalogNode[]): string | null {
		for (const node of list) {
			if (!node) continue;
			if (node.role === 'lesson' && playableLesson(node)) return node.path;
			const found = findLesson(node.children);
			if (found) return found;
		}
		return null;
	}
	function findLoose(list: CatalogNode[]): string | null {
		for (const node of list) {
			if (!node) continue;
			const loose = node.looseFiles.find(playableMedia);
			if (loose) return loose.path;
			const found = findLoose(node.children);
			if (found) return found;
		}
		return null;
	}
	return findLesson(nodes) ?? findLoose(nodes);
}

/**
 * Caminho que contém um `media_id` na subárvore: a aula de uma mídia de aula ou o
 * arquivo solto de um contêiner.
 *
 * Só devolve caminhos que `findLesson` consegue abrir e ignora mídia marcada como
 * ausente, para que o histórico recente nunca escolha uma aula sem arquivo.
 */
export function lessonPathForMedia(nodes: CatalogNode[], mediaId: string | null): string | null {
	if (!mediaId) return null;
	function visit(list: CatalogNode[]): string | null {
		for (const node of list) {
			if (!node) continue;
			if (node.role === 'lesson' && [...node.files, ...node.looseFiles].some((file) => playableMedia(file) && file.mediaId === mediaId)) return node.path;
			const loose = node.looseFiles.find((file) => playableMedia(file) && file.mediaId === mediaId);
			if (node.role !== 'lesson' && loose) return loose.path;
			const found = visit(node.children);
			if (found) return found;
		}
		return null;
	}
	return visit(nodes);
}

/**
 * Caminho inicial de um curso: a mídia mais recente do histórico que ainda existe no
 * curso ou, sem histórico utilizável, a primeira aula disponível.
 *
 * `mediaIds` chega do mais recente para o mais antigo. O histórico é do usuário, não do
 * curso, então as mídias de outros cursos simplesmente não encontram caminho aqui.
 */
export function defaultLessonPath(nodes: CatalogNode[], mediaIds: string[]): string | null {
	for (const mediaId of mediaIds) {
		const path = lessonPathForMedia(nodes, mediaId);
		if (path) return path;
	}
	return firstAvailableLessonPath(nodes);
}

/**
 * Contêiner visível mais próximo de uma aula ou mídia solta: módulo ou categoria.
 *
 * Serve para mostrar os materiais do módulo da aula aberta fora da playlist, sem repetir
 * esses materiais em todas as linhas da lista lateral. Aula diretamente sob o curso não
 * tem contêiner e devolve `null`.
 */
export function lessonContainer(nodes: CatalogNode[], lessonPath: string | null): CatalogNode | null {
	if (!lessonPath) return null;
	function visit(list: CatalogNode[], container: CatalogNode | null): CatalogNode | null {
		for (const node of list) {
			if (!node) continue;
			const next = node.role === 'module' || node.role === 'category' ? node : container;
			if (node.path === lessonPath) return container;
			if ([...node.files, ...node.looseFiles].some((file) => file.path === lessonPath)) {
				return node.role === 'module' || node.role === 'category' ? node : container;
			}
			const found = visit(node.children, next);
			if (found) return found;
		}
		return null;
	}
	return visit(nodes, null);
}
