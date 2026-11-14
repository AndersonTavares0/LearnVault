import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronRight, Folder, RefreshCw } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { transport, type RemoteSource, type SourceFolder } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys } from '@/app/data';

const browseKey = (sourceId: string) => ['source-browse', sourceId] as const;
const rootsKey = (sourceId: string) => ['source-roots', sourceId] as const;

const stateLabel = (state: SourceFolder['state']) => ({
	free: '',
	root: tr('Course', 'Curso'),
	inside_root: tr('Part of a course', 'Parte de um curso'),
	contains_root: tr('Has courses inside', 'Tem cursos dentro')
})[state];

/**
 * Choosing the courses of a remote source: the administrator walks the folders one level at a
 * time and picks the folder that is a course, or every folder inside one. Nothing is guessed and
 * nothing on the source is touched; every choice can be undone from the same list.
 */
export function SourceCourses({ source, onMessage }: { source: RemoteSource; onMessage: (message: string) => void }) {
	const client = useQueryClient();
	const [open, setOpen] = useState(false);
	const [path, setPath] = useState('');
	const [notice, setNotice] = useState('');
	// The previous folder stays on screen while the next one loads, so the list does not flash away and
	// the keyboard focus is not dropped to the page behind the dialog.
	const listing = useQuery({ queryKey: [...browseKey(source.id), path], queryFn: () => transport.browseSource!(source.id, path), enabled: open, retry: false, placeholderData: keepPreviousData });
	const pathBar = useRef<HTMLElement>(null);
	const walked = useRef(false);
	// Opening a folder replaces the buttons that were focused, so focus moves to the new level of the path.
	useEffect(() => {
		if (!walked.current) { walked.current = true; return; }
		pathBar.current?.querySelector<HTMLElement>('[aria-current="page"]')?.focus();
	}, [path]);
	const roots = useQuery({ queryKey: rootsKey(source.id), queryFn: () => transport.listSourceRoots!(source.id), enabled: open, retry: false });
	const changed = () => Promise.all([
		client.invalidateQueries({ queryKey: browseKey(source.id) }),
		client.invalidateQueries({ queryKey: rootsKey(source.id) }),
		client.invalidateQueries({ queryKey: keys.catalog })
	]);
	const choose = useMutation({
		mutationFn: (input: { path: string; mode: 'course' | 'children' }) => transport.chooseSourceRoots!(source.id, input.path, input.mode),
		onSuccess: (result) => {
			const made = formatCount(result.created.length, tr('course', 'curso'), tr('courses', 'cursos'));
			const skipped = result.skipped.length ? ' ' + tr(`${formatCount(result.skipped.length, 'folder was', 'folders were')} left out.`, `${formatCount(result.skipped.length, 'pasta ficou', 'pastas ficaram')} de fora.`) : '';
			setNotice(tr(`Chosen: ${made}.${skipped} Use “Read source” to load their lessons.`, `Escolhido: ${made}.${skipped} Use “Ler a fonte” para carregar as aulas.`));
			void changed();
		}
	});
	const remove = useMutation({
		mutationFn: (rootId: string) => transport.removeSourceRoot!(source.id, rootId),
		onSuccess: () => { setNotice(tr('The choice was undone. Its lessons went back to the source course.', 'A escolha foi desfeita. As aulas voltaram ao curso da fonte.')); void changed(); }
	});
	const scan = useMutation({
		mutationFn: () => transport.scanSource!(source.id),
		onSuccess: (summary) => {
			const lessons = formatCount(summary.inserted, tr('new lesson', 'nova aula'), tr('new lessons', 'novas aulas'));
			onMessage(summary.complete ? tr(`Source read: ${lessons}.`, `Fonte lida: ${lessons}.`) : tr(`Source read: ${lessons}. Some folders could not be read.`, `Fonte lida: ${lessons}. Algumas pastas não puderam ser lidas.`));
			void changed();
		},
		onError: (error) => onMessage(error.message)
	});

	const segments = path ? path.split('/') : [];
	const folders = listing.data?.folders ?? [];
	const freeFolders = folders.filter((folder) => folder.state === 'free');
	// What the list shows (not the path being opened) decides the label and the target of the bulk action.
	const shown = listing.data?.path ?? '';
	const here = shown ? shown.split('/').at(-1)! : source.name;
	const error = listing.error ?? roots.error ?? choose.error ?? remove.error;

	return (
		<>
			<Button variant="outline" size="sm" onClick={() => { setNotice(''); setOpen(true); }}>{tr('Choose courses', 'Escolher cursos')}</Button>
			<Button variant="outline" size="sm" disabled={scan.isPending} onClick={() => { onMessage(tr('Reading the source…', 'Lendo a fonte…')); scan.mutate(); }}>
				<RefreshCw className={cn(scan.isPending && 'motion-safe:animate-spin')} />{tr('Read source', 'Ler a fonte')}
			</Button>
			<Dialog open={open} onOpenChange={(next) => { setOpen(next); if (!next) { setPath(''); choose.reset(); remove.reset(); } }}>
				<DialogContent className="sm:max-w-2xl">
					<DialogHeader>
						<DialogTitle className="font-serif text-2xl">{tr(`Choose the courses of ${source.name}`, `Escolher os cursos de ${source.name}`)}</DialogTitle>
						<DialogDescription>{tr('Open folders to find each course, then choose it. Your files are only read, and you can undo any choice.', 'Abra as pastas para achar cada curso e escolha. Seus arquivos só são lidos, e você pode desfazer qualquer escolha.')}</DialogDescription>
					</DialogHeader>

					<nav ref={pathBar} aria-label={tr('Folder path', 'Caminho da pasta')} className="flex flex-wrap items-center gap-1 text-sm">
						<Button variant="ghost" size="sm" onClick={() => setPath('')} aria-current={path === '' ? 'page' : undefined}>{source.name}</Button>
						{segments.map((segment, index) => (
							<span key={segments.slice(0, index + 1).join('/')} className="flex items-center gap-1">
								<ChevronRight className="size-4 text-muted-foreground" aria-hidden="true" />
								<Button variant="ghost" size="sm" onClick={() => setPath(segments.slice(0, index + 1).join('/'))} aria-current={index === segments.length - 1 ? 'page' : undefined}>{segment}</Button>
							</span>
						))}
					</nav>

					{error && <p role="alert" className="text-sm text-destructive">{error.message}</p>}
					<p role="status" aria-live="polite" className="text-sm empty:hidden">{notice}</p>

					{listing.isPending ? (
						<p role="status" className="text-sm text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>
					) : listing.data && (
						<div className="space-y-3" aria-busy={listing.isPlaceholderData}>
							<p className="text-sm text-muted-foreground">{tr(`${formatCount(listing.data.mediaFiles, 'video or audio file', 'video or audio files')} directly in this folder.`, `${formatCount(listing.data.mediaFiles, 'arquivo de vídeo ou áudio', 'arquivos de vídeo ou áudio')} direto nesta pasta.`)}</p>
							{freeFolders.length > 1 && (
								<Button variant="outline" disabled={choose.isPending || listing.isPlaceholderData} onClick={() => choose.mutate({ path: shown, mode: 'children' })}>
									{tr(`Make each of the ${freeFolders.length} folders in “${here}” a course`, `Tornar cada uma das ${freeFolders.length} pastas de “${here}” um curso`)}
								</Button>
							)}
							{folders.length === 0 ? (
								<p className="rounded-lg border border-dashed p-3 text-sm text-muted-foreground">{tr('No folders here.', 'Nenhuma pasta aqui.')}</p>
							) : (
								<ul className="max-h-[40dvh] divide-y overflow-y-auto rounded-lg border bg-card">
									{folders.map((folder) => (
										<li key={folder.path} className="flex flex-wrap items-center gap-2 p-2">
											<Button variant="ghost" className="mr-auto min-w-0 justify-start" onClick={() => setPath(folder.path)} aria-label={tr(`Open ${folder.name}`, `Abrir ${folder.name}`)}>
												<Folder aria-hidden="true" /><span className="truncate">{folder.name}</span>
											</Button>
											{folder.state === 'free' ? (
												<Button variant="outline" size="sm" disabled={choose.isPending || listing.isPlaceholderData} onClick={() => choose.mutate({ path: folder.path, mode: 'course' })} aria-label={tr(`Choose ${folder.name} as a course`, `Escolher ${folder.name} como um curso`)}>{tr('Choose as course', 'Escolher como curso')}</Button>
											) : (
												<span className="text-sm text-muted-foreground">{stateLabel(folder.state)}</span>
											)}
										</li>
									))}
								</ul>
							)}
							{listing.data.truncated && <p className="text-sm text-muted-foreground">{tr('Only the first folders are shown. Open a folder to see its own.', 'Só as primeiras pastas são mostradas. Abra uma pasta para ver as dela.')}</p>}
						</div>
					)}

					<section aria-labelledby="chosen-courses" className="space-y-2">
						<h3 id="chosen-courses" className="font-medium">{tr('Chosen courses', 'Cursos escolhidos')}</h3>
						{(roots.data?.length ?? 0) === 0 ? (
							<p className="text-sm text-muted-foreground">{tr('None yet. Without a choice, the whole source is one course.', 'Nenhum ainda. Sem escolha, a fonte inteira é um curso só.')}</p>
						) : (
							<ul className="max-h-[25dvh] divide-y overflow-y-auto rounded-lg border bg-card">
								{roots.data!.map((root) => (
									<li key={root.id} className="flex flex-wrap items-center gap-2 p-2">
										<span className="mr-auto min-w-0"><span className="block font-medium break-words">{root.name}</span><span className="block text-xs break-all text-muted-foreground">{root.path}</span></span>
										<Button variant="ghost" size="sm" disabled={remove.isPending} onClick={() => remove.mutate(root.id)} aria-label={tr(`Undo the choice of ${root.name}`, `Desfazer a escolha de ${root.name}`)}>{tr('Undo', 'Desfazer')}</Button>
									</li>
								))}
							</ul>
						)}
					</section>
					<DialogFooter>
						<DialogClose render={<Button type="button" variant="outline" />}>{tr('Close', 'Fechar')}</DialogClose>
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</>
	);
}
