import type { CatalogNode } from './transport';

export type LessonRef = { title: string; path: string };
export type LessonGroup = { title: string | null; lessons: LessonRef[] };

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
 * Contêiner que agrupa a aula: o curso mais próximo, senão o módulo mais próximo.
 *
 * "Mais próximo de dentro para fora" por papel, e não por profundidade: uma aula em
 * `categoria/curso/módulo/aula` pertence ao curso, porque é o curso que agrupa os
 * módulos. Sem curso e sem módulo — uma aula solta direto sob a categoria — o grupo é o
 * catálogo inteiro, com título nulo, para o índice continuar util em vez de vazio.
 */
function groupingContainer(ancestors: CatalogNode[]): CatalogNode | null {
	for (const role of ['course', 'module']) {
		const found = [...ancestors].reverse().find((node) => node.role === role);
		if (found) return found;
	}
	return null;
}

/**
 * O índice de aulas exibido ao lado do player: as aulas do mesmo curso ou módulo, na
 * ordem canônica, com o título do contêiner quando ele existe.
 */
export function lessonGroup(nodes: CatalogNode[], lessonPath: string | null): LessonGroup {
	if (lessonPath === null) return { title: null, lessons: [] };
	const indexed = indexLessons(nodes);
	const current = indexed.find((entry) => entry.ref.path === lessonPath);
	if (!current) return { title: null, lessons: [] };
	const container = groupingContainer(current.ancestors);
	const lessons = container === null ? indexed.map((entry) => entry.ref) : indexed.filter((entry) => entry.ancestors.includes(container)).map((entry) => entry.ref);
	return { title: container?.title ?? null, lessons };
}
