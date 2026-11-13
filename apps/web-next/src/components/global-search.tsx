import { BookOpen, NotebookPen, Play, Search, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useSearch } from '@/app/data';
import { tr } from '@/lib/i18n';
import { formatDuration } from '@/lib/player';
import { href } from '@/lib/router';
import type { SearchCourseResult, SearchLessonResult, SearchNoteResult, SearchResults } from '@/lib/transport';
import { cn } from '@/lib/utils';

type Item =
	| { kind: 'note'; note: SearchNoteResult }
	| { kind: 'course'; course: SearchCourseResult }
	| { kind: 'lesson'; lesson: SearchLessonResult };

const EMPTY: SearchResults = { notes: [], courses: [], lessons: [] };

/**
 * Global search palette in the topbar. `/` and Ctrl/Cmd+K focus it, arrows move through
 * notes, courses and lessons, Enter opens the active result and Escape closes it.
 * Results render as text (snippets carry `<mark>` markers, never HTML).
 */
export function GlobalSearch() {
	const navigate = useNavigate();
	const input = useRef<HTMLInputElement>(null);
	const box = useRef<HTMLDivElement>(null);
	const [query, setQuery] = useState('');
	const [open, setOpen] = useState(false);
	// Phones have no room for the palette in the topbar, so the tab bar opens it full-screen.
	const [mobile, setMobile] = useState(false);
	const [active, setActive] = useState(-1);
	const ready = query.trim().length >= 2;
	const search = useSearch(query, open);
	const results = search.data ?? EMPTY;
	const items = useMemo<Item[]>(() => [
		...results.notes.map((note) => ({ kind: 'note' as const, note })),
		...results.courses.map((course) => ({ kind: 'course' as const, course })),
		...results.lessons.map((lesson) => ({ kind: 'lesson' as const, lesson }))
	], [results]);

	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			const target = event.target;
			const typing = target instanceof Element && Boolean(target.closest('input, textarea, select, [contenteditable="true"]'));
			const shortcut = (event.key === 'k' || event.key === 'K') && (event.ctrlKey || event.metaKey);
			if (!shortcut && (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || typing)) return;
			event.preventDefault();
			setOpen(true);
			input.current?.focus();
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	}, []);

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
			setMobile(true);
			setOpen(true);
			input.current?.focus();
		};
		window.addEventListener('learnvault:search', onOpen);
		return () => window.removeEventListener('learnvault:search', onOpen);
	}, []);

	const close = () => {
		setOpen(false);
		setMobile(false);
		setActive(-1);
		setQuery('');
	};

	const openItem = (item: Item) => {
		if (item.kind === 'note') navigate(href('course', item.note.courseId, item.note.lessonPath, item.note.positionSeconds).replace(/^#/, ''));
		else if (item.kind === 'lesson') navigate(href('course', item.lesson.courseId, item.lesson.path).replace(/^#/, ''));
		else navigate(href('course', item.course.courseId).replace(/^#/, ''));
		close();
		input.current?.blur();
	};

	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key === 'Escape') {
			event.preventDefault();
			close();
			input.current?.blur();
			return;
		}
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			if (!open) setOpen(true);
			if (items.length === 0) return;
			event.preventDefault();
			const step = event.key === 'ArrowDown' ? 1 : -1;
			setActive((current) => (current + step + items.length) % items.length);
			return;
		}
		if (event.key === 'Enter' && open && items.length > 0) {
			event.preventDefault();
			openItem(items[active >= 0 ? active : 0]);
		}
	};

	const showPanel = open && ready;
	const total = items.length;
	return (
		<div ref={box} className={cn('relative', mobile
			? 'fixed inset-0 z-50 flex flex-col bg-background p-4 md:static md:z-auto md:block md:w-44 md:bg-transparent md:p-0 lg:w-64 xl:w-72'
			: 'hidden w-44 md:block lg:w-64 xl:w-72')}>
			<div className="relative flex items-center">
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
					placeholder={tr('Search everything', 'Buscar em tudo')}
					onFocus={() => setOpen(true)}
					onChange={(event) => { setQuery(event.target.value); setOpen(true); setActive(-1); }}
					onKeyDown={onKeyDown}
					className={cn('pl-9', mobile && 'pr-12')}
				/>
				{mobile && <Button variant="ghost" size="icon" className="absolute top-0 right-0 md:hidden" onClick={() => { close(); input.current?.blur(); }} aria-label={tr('Close search', 'Fechar busca')}><X /></Button>}
			</div>
			{showPanel && (
				<div id="global-search-results" role="listbox" aria-label={tr('Search results', 'Resultados da busca')} className={cn('overflow-y-auto rounded-2xl border bg-popover p-2 shadow-lg', mobile ? 'mt-3 max-h-[calc(100dvh-6rem)]' : 'absolute top-full right-0 z-50 mt-2 max-h-[min(70vh,32rem)] w-[min(28rem,calc(100vw-2rem))]')}>
					{search.isPending && <p role="status" className="px-3 py-4 text-sm text-muted-foreground">{tr('Searching…', 'Buscando…')}</p>}
					{search.isError && <p role="alert" className="px-3 py-4 text-sm text-destructive">{tr('Search is unavailable right now.', 'A busca não está disponível agora.')}</p>}
					{!search.isPending && !search.isError && total === 0 && <p role="status" className="px-3 py-4 text-sm text-muted-foreground">{tr(`No results for “${query.trim()}”.`, `Nenhum resultado para “${query.trim()}”.`)}</p>}
					{!search.isPending && !search.isError && results.notes.length > 0 && (
						<Group label={tr('Notes', 'Notas')}>
							{results.notes.map((note, index) => (
								<Option key={note.noteId} id={`global-search-${index}`} active={active === index} onClick={() => openItem({ kind: 'note', note })}>
									<span className="mt-0.5 text-primary" aria-hidden="true"><NotebookPen className="size-4" /></span>
									<span className="min-w-0 flex-1 space-y-0.5">
										<span className="flex flex-wrap items-baseline gap-x-2">
											<span className="font-medium">{note.lessonTitle}</span>
											<span className="text-xs text-muted-foreground">{note.courseTitle} · {formatDuration(note.positionSeconds)}</span>
										</span>
										<Marked text={note.snippet} />
									</span>
								</Option>
							))}
						</Group>
					)}
					{!search.isPending && !search.isError && results.courses.length > 0 && (
						<Group label={tr('Courses', 'Cursos')}>
							{results.courses.map((course, offset) => {
								const index = results.notes.length + offset;
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
								const index = results.notes.length + results.courses.length + offset;
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
	);
}

function Group({ label, children }: { label: string; children: ReactNode }) {
	return (
		<div role="group" aria-label={label} className="py-1">
			<p className="px-3 py-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase">{label}</p>
			{children}
		</div>
	);
}

function Option({ id, active, onClick, children }: { id: string; active: boolean; onClick: () => void; children: ReactNode }) {
	return (
		<button id={id} type="button" role="option" aria-selected={active} tabIndex={-1} onClick={onClick} className={cn('flex w-full min-h-11 items-start gap-2 rounded-xl px-3 py-2 text-left text-sm hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', active && 'bg-panel')}>
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
