import { Bookmark, BookOpen, Layers, NotebookPen, Play, Search, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useSearch } from '@/app/data';
import { tr } from '@/lib/i18n';
import { formatDuration } from '@/lib/player';
import { href } from '@/lib/router';
import type { SearchBookmarkResult, SearchCourseResult, SearchFlashcardResult, SearchLessonResult, SearchNoteResult, SearchResults } from '@/lib/transport';
import { cn } from '@/lib/utils';

type Item =
	| { kind: 'note'; note: SearchNoteResult }
	| { kind: 'flashcard'; card: SearchFlashcardResult }
	| { kind: 'bookmark'; mark: SearchBookmarkResult }
	| { kind: 'course'; course: SearchCourseResult }
	| { kind: 'lesson'; lesson: SearchLessonResult };

const EMPTY: SearchResults = { notes: [], courses: [], lessons: [], flashcards: [], bookmarks: [] };

/** Below `lg` the rail field is display:none, so shortcuts must open the phone palette instead. */
const isNarrow = () => window.matchMedia?.('(min-width: 1024px)')?.matches === false;

/**
 * Global search palette: the desktop rail field, or a full-screen sheet on phones. `/` and Ctrl/Cmd+K focus it, arrows move through
 * notes, courses and lessons, Enter opens the active result and Escape closes it.
 * Results render as text (snippets carry `<mark>` markers, never HTML).
 */
export function GlobalSearch({ collapsed = false, shortcut }: { collapsed?: boolean; shortcut?: string } = {}) {
	const navigate = useNavigate();
	const input = useRef<HTMLInputElement>(null);
	const closeButton = useRef<HTMLButtonElement>(null);
	const box = useRef<HTMLDivElement>(null);
	const [query, setQuery] = useState('');
	const [open, setOpen] = useState(false);
	// Phones (and collapsed desktop rail) open a full-viewport command palette sheet.
	const [mobile, setMobile] = useState(false);
	const [active, setActive] = useState(-1);
	// The control that opened the palette, so dismissal can hand focus back to it.
	const opener = useRef<HTMLElement | null>(null);
	const ready = query.trim().length >= 2;
	const search = useSearch(query, open);
	const results = search.data ?? EMPTY;
	const flashcards = useMemo(() => results.flashcards ?? [], [results.flashcards]);
	const bookmarks = useMemo(() => results.bookmarks ?? [], [results.bookmarks]);
	const items = useMemo<Item[]>(() => [
		...results.notes.map((note) => ({ kind: 'note' as const, note })),
		...flashcards.map((card) => ({ kind: 'flashcard' as const, card })),
		...bookmarks.map((mark) => ({ kind: 'bookmark' as const, mark })),
		...results.courses.map((course) => ({ kind: 'course' as const, course })),
		...results.lessons.map((lesson) => ({ kind: 'lesson' as const, lesson }))
	], [results.notes, results.courses, results.lessons, flashcards, bookmarks]);

	useEffect(() => {
		if (active < 0) return;
		document.getElementById(`global-search-${active}`)?.scrollIntoView?.({ block: 'nearest' });
	}, [active]);

	const rememberOpener = () => {
		if (!box.current?.contains(document.activeElement)) opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
	};

	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			const target = event.target;
			const typing = target instanceof Element && Boolean(target.closest('input, textarea, select, [contenteditable="true"]'));
			const isShortcut = (event.key === 'k' || event.key === 'K') && (event.ctrlKey || event.metaKey);
			if (!isShortcut && (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || typing)) return;
			event.preventDefault();
			rememberOpener();
			setOpen(true);
			if (isNarrow() || collapsed) setMobile(true);
			input.current?.focus();
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	}, [collapsed]);

	useEffect(() => {
		const onPointer = (event: PointerEvent) => {
			if (!box.current?.contains(event.target as Node)) setOpen(false);
		};
		window.addEventListener('pointerdown', onPointer);
		return () => window.removeEventListener('pointerdown', onPointer);
	}, []);

	// The bottom tab bar asks for the palette; it is the only search entry on a phone.
	useEffect(() => {
		const onOpen = () => {
			rememberOpener();
			setMobile(true);
			setOpen(true);
		};
		window.addEventListener('learnvault:search', onOpen);
		return () => window.removeEventListener('learnvault:search', onOpen);
	}, []);

	// The phone overlay is display:none until `mobile` commits, so focus can only land after that render.
	useEffect(() => {
		if (mobile) input.current?.focus();
	}, [mobile]);

	const close = (restoreFocus = false) => {
		const back = opener.current;
		opener.current = null;
		setOpen(false);
		setMobile(false);
		setActive(-1);
		setQuery('');
		if (restoreFocus) back?.focus();
	};

	const openItem = (item: Item) => {
		if (item.kind === 'note') navigate(href('course', item.note.courseId, item.note.lessonPath, item.note.positionSeconds).replace(/^#/, ''));
		else if (item.kind === 'flashcard') navigate(href('course', item.card.courseId, item.card.lessonPath, item.card.positionSeconds).replace(/^#/, ''));
		else if (item.kind === 'bookmark') navigate(href('course', item.mark.courseId, item.mark.lessonPath, item.mark.positionSeconds).replace(/^#/, ''));
		else if (item.kind === 'lesson') navigate(href('course', item.lesson.courseId, item.lesson.path).replace(/^#/, ''));
		else navigate(href('course', item.course.courseId).replace(/^#/, ''));
		close();
		input.current?.blur();
	};

	// The phone overlay's only tab stops are the field and the close button, so Tab wraps between them.
	const dismiss = (event: KeyboardEvent<HTMLElement>) => {
		if (event.key !== 'Escape') return false;
		event.preventDefault();
		close(true);
		input.current?.blur();
		return true;
	};

	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (dismiss(event)) return;
		if (event.key === 'Tab' && event.shiftKey && mobile) {
			event.preventDefault();
			closeButton.current?.focus();
			return;
		}
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			if (!open) setOpen(true);
			if (items.length === 0) return;
			event.preventDefault();
			const forward = event.key === 'ArrowDown';
			setActive((current) => {
				if (current < 0) return forward ? 0 : items.length - 1;
				return (current + (forward ? 1 : -1) + items.length) % items.length;
			});
			return;
		}
		if (event.key === 'Enter' && open && items.length > 0) {
			event.preventDefault();
			openItem(items[active >= 0 ? active : 0]);
		}
	};

	const showPanel = open && ready;
	const total = items.length;
	const overlay = mobile;
	return (
		<>
			{collapsed && !overlay && (
				<Button
					type="button"
					variant="ghost"
					size="icon"
					title={shortcut ? `${tr('Search', 'Buscar')} (${shortcut})` : tr('Search', 'Buscar')}
					aria-label={tr('Open search', 'Abrir busca')}
					onClick={() => {
						rememberOpener();
						setMobile(true);
						setOpen(true);
					}}
					className="hidden shrink-0 text-muted-foreground hover:text-foreground lg:inline-flex"
				>
					<Search />
				</Button>
			)}
			<div ref={box} className={cn('relative', overlay
				? 'fixed inset-0 z-50 flex flex-col bg-background/95 p-4 backdrop-blur-xs lg:items-center lg:pt-[10vh]'
				: collapsed ? 'hidden' : 'hidden w-44 lg:block lg:w-full')}>
				<div className={cn('relative flex items-center', overlay && 'w-full lg:max-w-2xl')}>
					<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
					<Input
						ref={input}
						type="search"
						value={query}
						role="combobox"
						aria-expanded={showPanel}
						aria-controls="global-search-results"
						aria-autocomplete="list"
						aria-activedescendant={showPanel && active >= 0 ? `global-search-${active}` : undefined}
						aria-label={tr('Search notes, courses and lessons', 'Buscar notas, cursos e aulas')}
						aria-keyshortcuts="/ Control+K Meta+K"
						placeholder={overlay ? tr('Search courses, lessons and notes', 'Buscar cursos, aulas e notas') : tr('Search', 'Buscar')}
						onFocus={() => setOpen(true)}
						onChange={(event) => { setQuery(event.target.value); setOpen(true); setActive(-1); }}
						onKeyDown={onKeyDown}
						className={cn('pl-9 [&::-webkit-search-cancel-button]:hidden', overlay ? 'pr-12' : query ? 'pr-10' : 'pr-14')}
					/>
					{!overlay && !query && shortcut && (
						<kbd aria-hidden="true" className="pointer-events-none absolute top-1/2 right-2.5 hidden -translate-y-1/2 rounded border border-border bg-panel px-1.5 py-0.5 font-sans text-[11px] font-medium text-muted-foreground lg:block">{shortcut}</kbd>
					)}
					{!overlay && query && (
						<Button type="button" variant="ghost" size="icon-xs" onClick={() => { setQuery(''); setActive(-1); input.current?.focus(); }} aria-label={tr('Clear search', 'Limpar busca')} className="absolute top-1/2 right-1 hidden -translate-y-1/2 text-muted-foreground hover:text-foreground lg:inline-flex">
							<X />
						</Button>
					)}
					{overlay && <Button variant="ghost" size="icon" className="absolute top-0 right-0" ref={closeButton} onKeyDown={(event) => { if (dismiss(event)) return; if (event.key === 'Tab' && !event.shiftKey) { event.preventDefault(); input.current?.focus(); } }} onClick={() => { close(true); input.current?.blur(); }} aria-label={tr('Close search', 'Fechar busca')}><X /></Button>}
				</div>
				{overlay && !ready && (
					<div className="mt-3 w-full rounded-[var(--radius)] border border-[var(--study-rule)] bg-popover px-4 py-3 text-sm text-muted-foreground lg:max-w-2xl lg:shadow-xl">
						{tr('Type at least 2 characters to search courses, lessons, and notes.', 'Digite ao menos 2 caracteres para buscar cursos, aulas e notas.')}
					</div>
				)}
				{showPanel && (
					<div id="global-search-results" role="listbox" aria-label={tr('Search results', 'Resultados da busca')} className={cn('overflow-y-auto rounded-[var(--radius)] border border-[var(--study-rule)] bg-popover p-2', overlay ? 'mt-3 w-full max-h-[calc(100dvh-6rem)] lg:max-w-2xl lg:shadow-xl' : 'absolute top-0 left-[calc(100%+0.75rem)] z-50 max-h-[min(75vh,34rem)] w-[min(30rem,calc(100vw-17rem))] shadow-xl')}>
					{search.isPending && <p role="status" className="px-3 py-4 text-sm text-muted-foreground">{tr('Searching…', 'Buscando…')}</p>}
					{search.isError && <p role="alert" className="px-3 py-4 text-sm text-destructive">{tr('Search is unavailable right now.', 'A busca não está disponível agora.')}</p>}
					{!search.isPending && !search.isError && total === 0 && <p role="status" className="px-3 py-4 text-sm text-muted-foreground">{tr(`No results for “${query.trim()}”.`, `Nenhum resultado para “${query.trim()}”.`)}</p>}
					{!search.isPending && !search.isError && total > 0 && (
						<p className="border-b border-[var(--study-rule)] px-3 pt-1 pb-2 text-xs text-muted-foreground tabular-nums">{total === 1 ? tr('1 result', '1 resultado') : tr(`${total} results`, `${total} resultados`)}</p>
					)}
					{!search.isPending && !search.isError && results.notes.length > 0 && (
						<Group label={tr('Notes', 'Notas')}>
							{results.notes.map((note, index) => (
								<Option key={note.noteId} id={`global-search-${index}`} active={active === index} onClick={() => openItem({ kind: 'note', note })}>
									<span className="mt-0.5 text-primary" aria-hidden="true"><NotebookPen className="size-4" /></span>
									<span className="min-w-0 flex-1 space-y-0.5">
										<span className="flex flex-wrap items-baseline gap-x-2">
											<span className="font-medium">{note.lessonTitle}</span>
											<span className="text-xs text-muted-foreground">{note.courseTitle} · {tr(`Opens at ${formatDuration(note.positionSeconds)}`, `Abre em ${formatDuration(note.positionSeconds)}`)}</span>
										</span>
										<Marked text={note.snippet} />
									</span>
								</Option>
							))}
						</Group>
					)}
					{!search.isPending && !search.isError && flashcards.length > 0 && (
						<Group label={tr('Flashcards', 'Cartões')}>
							{flashcards.map((card, offset) => {
								const index = results.notes.length + offset;
								return (
									<Option key={card.cardId} id={`global-search-${index}`} active={active === index} onClick={() => openItem({ kind: 'flashcard', card })}>
										<span className="mt-0.5 text-primary" aria-hidden="true"><Layers className="size-4" /></span>
										<span className="min-w-0 flex-1 space-y-0.5">
											<span className="flex flex-wrap items-baseline gap-x-2">
												<span className="font-medium">{card.lessonTitle}</span>
												<span className="text-xs text-muted-foreground">{card.courseTitle}{card.positionSeconds !== null ? ` · ${formatDuration(card.positionSeconds)}` : ''}</span>
											</span>
											<Marked text={card.snippet} />
										</span>
									</Option>
								);
							})}
						</Group>
					)}
					{!search.isPending && !search.isError && bookmarks.length > 0 && (
						<Group label={tr('Marks', 'Marcadores')}>
							{bookmarks.map((mark, offset) => {
								const index = results.notes.length + flashcards.length + offset;
								return (
									<Option key={mark.bookmarkId} id={`global-search-${index}`} active={active === index} onClick={() => openItem({ kind: 'bookmark', mark })}>
										<span className="mt-0.5 text-primary" aria-hidden="true"><Bookmark className="size-4" /></span>
										<span className="min-w-0 flex-1 space-y-0.5">
											<span className="flex flex-wrap items-baseline gap-x-2">
												<span className="font-medium">{mark.lessonTitle}</span>
												<span className="text-xs text-muted-foreground">{mark.courseTitle} · {mark.kind === 'pdf' ? tr(`Page ${mark.positionSeconds}`, `Página ${mark.positionSeconds}`) : formatDuration(mark.positionSeconds)}</span>
											</span>
											<Marked text={mark.snippet} />
										</span>
									</Option>
								);
							})}
						</Group>
					)}
					{!search.isPending && !search.isError && results.courses.length > 0 && (
						<Group label={tr('Courses', 'Cursos')}>
							{results.courses.map((course, offset) => {
								const index = results.notes.length + flashcards.length + bookmarks.length + offset;
								return (
									<Option key={course.courseId} id={`global-search-${index}`} active={active === index} onClick={() => openItem({ kind: 'course', course })}>
										<span className="mt-0.5 text-primary" aria-hidden="true"><BookOpen className="size-4" /></span>
										<span className="min-w-0 flex-1"><span className="block font-medium">{course.title}</span><span className="text-xs text-muted-foreground">{tr('Course', 'Curso')}</span></span>
									</Option>
								);
							})}
						</Group>
					)}
					{!search.isPending && !search.isError && results.lessons.length > 0 && (
						<Group label={tr('Lessons', 'Aulas')}>
							{results.lessons.map((lesson, offset) => {
								const index = results.notes.length + flashcards.length + bookmarks.length + results.courses.length + offset;
								return (
									<Option key={lesson.mediaId} id={`global-search-${index}`} active={active === index} onClick={() => openItem({ kind: 'lesson', lesson })}>
										<span className="mt-0.5 text-primary" aria-hidden="true"><Play className="size-4" /></span>
										<span className="min-w-0 flex-1"><span className="block font-medium">{lesson.title}</span><span className="text-xs text-muted-foreground">{lesson.courseTitle}</span></span>
									</Option>
								);
							})}
						</Group>
					)}
				</div>
			)}
			</div>
		</>
	);
}

function Group({ label, children }: { label: string; children: ReactNode }) {
	return (
		<div role="group" aria-label={label} className="py-1 [&+&]:border-t [&+&]:border-[var(--study-rule)]">
			<p className="px-3 py-1 text-xs font-semibold text-muted-foreground">{label}</p>
			{children}
		</div>
	);
}

function Option({ id, active, onClick, children }: { id: string; active: boolean; onClick: () => void; children: ReactNode }) {
	return (
		<button id={id} type="button" role="option" aria-selected={active} tabIndex={-1} onClick={onClick} className={cn('flex w-full min-h-11 items-start gap-2 rounded-md px-3 py-2 text-left text-sm [overflow-wrap:anywhere] hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', active && 'bg-panel outline-2 -outline-offset-2 outline-ring')}>
			{children}
		</button>
	);
}

/** Splits an FTS5 snippet into plain and highlighted segments. */
function segments(text: string): { text: string; marked: boolean }[] {
	const result: { text: string; marked: boolean }[] = [];
	let marked = false;
	for (const part of text.split(/(<mark>|<\/mark>)/)) {
		if (part === '<mark>') marked = true;
		else if (part === '</mark>') marked = false;
		else if (part) result.push({ text: part, marked });
	}
	return result;
}

/** Renders FTS5 snippet markers as `<mark>` without injecting HTML. */
function Marked({ text }: { text: string }) {
	return (
		<span className="line-clamp-2 block text-xs text-muted-foreground">
			{segments(text).map((part, index) => part.marked
				? <mark key={index} className="rounded-sm bg-reward-fill/30 px-0.5 text-foreground">{part.text}</mark>
				: <span key={index}>{part.text}</span>)}
		</span>
	);
}
