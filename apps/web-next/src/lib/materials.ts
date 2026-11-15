import { normalizeForSearch } from './search';
import type { FileEntry } from './transport';

/** More materials than this are grouped by folder; fewer stay one plain list. */
export const GROUP_FROM = 12;

export type MaterialGroup = { key: string; label: string; files: FileEntry[] };

const natural = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

const folderOf = (path: string) => path.split('/').slice(0, -1);

/**
 * Materials grouped by the folder they live in, in natural order ("2" before "10"). The folders
 * every file shares are left out of the label, so a group reads "03 Calculus / 02 Limits" and not
 * the whole path. Files at the shared root form a group of their own, with an empty label.
 */
export function groupMaterials(files: FileEntry[]): MaterialGroup[] {
	const folders = files.map((file) => folderOf(file.path));
	let shared = folders[0]?.length ?? 0;
	for (const folder of folders) {
		let at = 0;
		while (at < shared && at < folder.length && folder[at] === folders[0][at]) at += 1;
		shared = at;
	}
	const groups = new Map<string, MaterialGroup>();
	files.forEach((file, index) => {
		const key = folders[index].join('/');
		const group = groups.get(key) ?? { key, label: folders[index].slice(shared).join(' / '), files: [] };
		group.files.push(file);
		groups.set(key, group);
	});
	return [...groups.values()]
		.map((group) => ({ ...group, files: group.files.sort((a, b) => natural.compare(a.name, b.name)) }))
		.sort((a, b) => natural.compare(a.key, b.key));
}

/** The files whose name or folder holds every word of `query`, ignoring case and accents. */
export function filterMaterials(files: FileEntry[], query: string): FileEntry[] {
	const words = normalizeForSearch(query).split(/\s+/).filter(Boolean);
	if (words.length === 0) return files;
	return files.filter((file) => {
		const text = normalizeForSearch(file.path);
		return words.every((word) => text.includes(word));
	});
}
