import type { CatalogNode } from './transport';

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
