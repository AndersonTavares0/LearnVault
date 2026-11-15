import { ChevronRight, Download, Search } from 'lucide-react';
import { useMemo, useState } from 'react';
import { buttonVariants } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { tr } from '@/lib/i18n';
import { filterMaterials, GROUP_FROM, groupMaterials } from '@/lib/materials';
import { fileStatus, transport, type FileEntry } from '@/lib/transport';

function MaterialRow({ file, courseId }: { file: FileEntry; courseId: string }) {
	const url = file.availability?.trim().toLowerCase() === 'missing' ? null : transport.mediaUrl(file, courseId);
	const status = fileStatus(file);
	return (
		<li className="flex min-h-14 items-center gap-3 px-4 py-2">
			<span className="min-w-0 flex-1 break-words">{file.name}{status && <span className="ml-2 text-xs text-muted-foreground">({status})</span>}</span>
			{url && (
				<a href={url} download={file.name} target="_blank" rel="noopener noreferrer" className={buttonVariants({ variant: 'outline', size: 'sm' })} aria-label={tr(`Download ${file.name}`, `Baixar ${file.name}`)}>
					<Download />{tr('Download', 'Baixar')}
				</a>
			)}
		</li>
	);
}

/**
 * Non-media files of a course or lesson, each with a download link when on disk. A long list is
 * grouped by folder, closed until opened, and can be searched, so a course with thousands of files
 * stays readable.
 */
export function Materials({ files, courseId }: { files: FileEntry[]; courseId: string }) {
	const [query, setQuery] = useState('');
	const shown = useMemo(() => filterMaterials(files, query), [files, query]);
	const groups = useMemo(() => (files.length > GROUP_FROM ? groupMaterials(shown) : []), [files.length, shown]);
	if (!files.length) return <p className="text-muted-foreground">{tr('No support materials in this course.', 'Nenhum material de apoio neste curso.')}</p>;
	if (files.length <= GROUP_FROM) {
		return <ul className="divide-y rounded-lg border bg-card">{files.map((file) => <MaterialRow key={file.path} file={file} courseId={courseId} />)}</ul>;
	}
	const searching = query.trim().length > 0;
	return (
		<div className="space-y-4">
			<div className="relative max-w-md">
				<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
				<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} className="pl-9" aria-label={tr('Search materials', 'Buscar materiais')} placeholder={tr('Search materials…', 'Buscar materiais…')} />
			</div>
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground">{searching ? tr(`${shown.length} of ${files.length} materials, in ${groups.length} folders`, `${shown.length} de ${files.length} materiais, em ${groups.length} pastas`) : tr(`${files.length} materials in ${groups.length} folders`, `${files.length} materiais em ${groups.length} pastas`)}</p>
			{groups.length === 0 ? <p className="text-muted-foreground">{tr('No material matches the search.', 'Nenhum material corresponde à busca.')}</p> : (
				<div className="space-y-2">
					{groups.map((group) => (
						<details key={group.key} open={searching || groups.length === 1} className="group rounded-lg border bg-card">
							<summary className="flex min-h-12 cursor-pointer list-none items-center gap-2 px-4 py-2 font-medium focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
								<ChevronRight className="size-4 shrink-0 transition-transform group-open:rotate-90" aria-hidden="true" />
								<span className="min-w-0 flex-1 break-words">{group.label || tr('Course folder', 'Pasta do curso')}</span>
								<span className="text-sm text-muted-foreground tabular-nums">{group.files.length}</span>
							</summary>
							<ul className="divide-y border-t">{group.files.map((file) => <MaterialRow key={file.path} file={file} courseId={courseId} />)}</ul>
						</details>
					))}
				</div>
			)}
		</div>
	);
}
