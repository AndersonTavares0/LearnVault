import type { CatalogNode } from './transport';

export type FileCounts = { videos: number; materials: number };
export type CatalogCounts = FileCounts & { modules: number; lessons: number };

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
 * Contagens de um nó e de toda a sua subárvore.
 *
 * `modules` e `lessons` contam nós por papel; `videos` e `materials` somam os arquivos do
 * nó e dos descendentes. O próprio nó entra na contagem quando é uma aula, para que a meta
 * de um curso descreva a subárvore inteira.
 */
export function countCatalogNode(node: CatalogNode): CatalogCounts {
	const counts: CatalogCounts = { modules: 0, lessons: 0, videos: 0, materials: 0 };
	function walk(current: CatalogNode): void {
		if (current.role === 'module') counts.modules += 1;
		if (current.role === 'lesson') counts.lessons += 1;
		const files = countCatalogFiles(current);
		counts.videos += files.videos;
		counts.materials += files.materials;
		for (const child of current.children) walk(child);
	}
	walk(node);
	return counts;
}

export function formatCount(value: number, singular: string, plural: string): string {
	return `${value} ${value === 1 ? singular : plural}`;
}

function joinParts(parts: string[]): string {
	return parts.filter(Boolean).join(' · ');
}

/** Meta de um curso: módulos, aulas e materiais, omitindo as partes zeradas. */
export function formatCourseMeta(counts: CatalogCounts): string {
	return joinParts([
		counts.modules > 0 ? formatCount(counts.modules, 'módulo', 'módulos') : '',
		counts.lessons > 0 ? formatCount(counts.lessons, 'aula', 'aulas') : '',
		counts.materials > 0 ? formatCount(counts.materials, 'material', 'materiais') : ''
	]);
}

/** Meta de uma linha de aula: vídeos e materiais do próprio nó, omitindo as partes zeradas. */
export function formatLessonMeta(counts: FileCounts): string {
	return joinParts([
		counts.videos > 0 ? formatCount(counts.videos, 'vídeo', 'vídeos') : '',
		counts.materials > 0 ? formatCount(counts.materials, 'material', 'materiais') : ''
	]);
}
