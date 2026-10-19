import { describe, expect, it } from 'vitest';
import { MOVE_DOWN, MOVE_UP, canMove, moveSibling, siblingIndex } from './order';
import { lessonSequence, neighboringLessons } from './navigation';
import type { CatalogNode } from './transport';

describe('manual ordering of siblings', () => {
	const order = ['Modulo 2', 'Modulo 3', 'Modulo 10'];

	it('moves a sibling up and down by one position without mutating the input', () => {
		const up = moveSibling(order, 'Modulo 3', MOVE_UP);
		expect(up).toEqual(['Modulo 3', 'Modulo 2', 'Modulo 10']);
		const down = moveSibling(order, 'Modulo 2', MOVE_DOWN);
		expect(down).toEqual(['Modulo 3', 'Modulo 2', 'Modulo 10']);
		expect(order).toEqual(['Modulo 2', 'Modulo 3', 'Modulo 10']);
	});

	it('returns the same reference when there is nothing to move', () => {
		expect(moveSibling(order, 'Modulo 2', MOVE_UP)).toBe(order);
		expect(moveSibling(order, 'Modulo 10', MOVE_DOWN)).toBe(order);
		expect(moveSibling(order, 'ausente', MOVE_DOWN)).toBe(order);
		expect(moveSibling(order, 'Modulo 2', 0)).toBe(order);
	});

	it('reports where a sibling sits and whether it can move', () => {
		expect(siblingIndex(order, 'Modulo 3')).toBe(1);
		expect(siblingIndex(order, 'ausente')).toBe(-1);
		expect(canMove(order, 'Modulo 2', MOVE_UP)).toBe(false);
		expect(canMove(order, 'Modulo 2', MOVE_DOWN)).toBe(true);
		expect(canMove(order, 'Modulo 10', MOVE_DOWN)).toBe(false);
		expect(canMove(order, 'ausente', MOVE_UP)).toBe(false);
	});
});

describe('navigation follows the catalog order after a manual reorder', () => {
	function lesson(title: string, path: string): CatalogNode {
		return { title, path, role: 'lesson', files: [], looseFiles: [], children: [] };
	}

	it('walks previous and next in the reordered sequence the tree already carries', () => {
		const course: CatalogNode = {
			title: 'Curso',
			path: '',
			role: 'course',
			files: [],
			looseFiles: [],
			children: [lesson('Aula 10', 'mod/aula-10'), lesson('Aula 2', 'mod/aula-2'), lesson('Aula 3', 'mod/aula-3')]
		};
		// A ordem manual do curso é a ordem dos filhos na árvore; a navegação a respeita.
		expect(lessonSequence([course]).map((ref) => ref.title)).toEqual(['Aula 10', 'Aula 2', 'Aula 3']);
		expect(neighboringLessons([course], 'mod/aula-2')).toEqual({
			previous: { title: 'Aula 10', path: 'mod/aula-10' },
			next: { title: 'Aula 3', path: 'mod/aula-3' }
		});
	});
});
