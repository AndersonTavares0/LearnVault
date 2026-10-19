/**
 * Ordenação manual de módulos e aulas entre irmãos.
 *
 * A árvore do catálogo já chega na ordem canônica (automática ou manual, resolvida pelo
 * Rust). Este módulo só calcula a próxima lista de irmãos a enviar ao persistir: mover um
 * irmão uma posição. Manter a lógica pura e testável evita duplicar a regra na interface.
 */

/** Nomes das ações de movimento, na ordem "para cima" e "para baixo". */
export const MOVE_UP = -1;
export const MOVE_DOWN = 1;

/**
 * Nova ordem de uma lista de irmãos após mover `path` em `delta` posições.
 *
 * `delta` negativo sobe e positivo desce. Um caminho ausente, um `delta` zero ou um
 * movimento que sairia da lista devolvem a **mesma** referência (nada a persistir); um
 * movimento válido devolve uma **nova** lista, sem mutar a original.
 */
export function moveSibling(order: string[], path: string, delta: number): string[] {
	const index = order.indexOf(path);
	if (index < 0 || delta === 0) return order;
	const target = index + delta;
	if (target < 0 || target >= order.length) return order;
	const next = order.slice();
	next[index] = order[target];
	next[target] = order[index];
	return next;
}

/** Índice de um irmão na lista, ou -1 quando ausente. */
export function siblingIndex(order: string[], path: string): number {
	return order.indexOf(path);
}

/** Verdadeiro quando o irmão pode se mover em `delta` (existe vaga na direção). */
export function canMove(order: string[], path: string, delta: number): boolean {
	const index = siblingIndex(order, path);
	if (index < 0) return false;
	const target = index + delta;
	return target >= 0 && target < order.length;
}
