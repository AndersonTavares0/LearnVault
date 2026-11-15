import { describe, expect, it } from 'vitest';
import { filterMaterials, groupMaterials } from './materials';
import type { FileEntry } from './transport';

const file = (path: string): FileEntry => ({ path, name: path.split('/').pop()!, size: 1 });

describe('groupMaterials', () => {
	const files = [file('Calc/10 Series/notes.pdf'), file('Calc/2 Limits/b.pdf'), file('Calc/2 Limits/a10.pdf'), file('Calc/2 Limits/a2.pdf'), file('Calc/readme.txt')];
	it('groups by folder, drops the shared folders from the label and sorts naturally', () => {
		const groups = groupMaterials(files);
		expect(groups.map((group) => group.label)).toEqual(['', '2 Limits', '10 Series']);
		expect(groups[1].files.map((item) => item.name)).toEqual(['a2.pdf', 'a10.pdf', 'b.pdf']);
	});
	it('keeps a single folder as one group with an empty label', () => {
		expect(groupMaterials([file('A/x.pdf'), file('A/y.pdf')]).map((group) => group.label)).toEqual(['']);
	});
});

describe('filterMaterials', () => {
	const files = [file('Física/Óptica/espelhos.pdf'), file('Física/Calor/Lista.pdf')];
	it('matches every word in the path, ignoring case and accents', () => {
		expect(filterMaterials(files, 'optica ESPELHOS').map((item) => item.name)).toEqual(['espelhos.pdf']);
		expect(filterMaterials(files, '  ')).toHaveLength(2);
		expect(filterMaterials(files, 'zzz')).toHaveLength(0);
	});
});
