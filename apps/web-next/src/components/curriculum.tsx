import { Check, ChevronDown, Star, TriangleAlert } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { courseProgress, isLessonMissing } from '@/lib/catalog';
import { lessonPdfId } from '@/lib/pdf';
import { tr } from '@/lib/i18n';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import type { CatalogNode } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useSetLessonCompleted, useToggleLessonFavorite } from '@/app/data';

type Props = {
	nodes: CatalogNode[];
	courseId: string;
	completed: ReadonlySet<string>;
	favorites: ReadonlySet<string>;
	/** Lesson open in the player: highlighted, and its modules start expanded. */
	activePath?: string | null;
	/** Bumping `n` opens or closes every module at once (the Expand all / Collapse all buttons). */
	signal?: { open: boolean; n: number };
	/** Tighter rows for the player sidebar: shorter lessons and modules. */
	compact?: boolean;
	/** Saved position of the lesson to resume (the active row); shown as "Resume at m:ss" when set. */
	resumeSeconds?: number | null;
};

type NodeProps = Props & { numbers: ReadonlyMap<string, number> };

export const lessonMediaId = (node: CatalogNode) => [...node.files, ...node.looseFiles].find((file) => file.kind === 'media' && file.mediaId)?.mediaId ?? lessonPdfId(node);
// Walks the children rather than trusting path prefixes: on a course picked inside an SMB or
// WebDAV source a module's path is relative to the course while its lessons keep the full path.
const contains = (node: CatalogNode, path: string | null | undefined): boolean => Boolean(path) && (node.path === path || path!.startsWith(`${node.path}/`) || node.children.some((child) => contains(child, path)));
/** Course-wide lesson numbers, so a module's lessons continue the count from the previous module. */
const lessonNumbers = (nodes: CatalogNode[], numbers = new Map<string, number>()): Map<string, number> => {
	for (const node of nodes) {
		if (node.role === 'lesson') numbers.set(node.path, numbers.size + 1);
		else lessonNumbers(node.children, numbers);
	}
	return numbers;
};
const chevron = 'size-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-[cubic-bezier(0.23,1,0.32,1)] group-open:rotate-180';

/**
 * Course curriculum as a tracklist: modules are native disclosures (keyboard and screen-reader
 * support for free, #321), lessons show their number, completion state and favourite. Shared by
 * the course page and the player sidebar.
 */
export function Curriculum(props: Props) {
	const numbers = lessonNumbers(props.nodes);
	return (
		<ul className={cn(props.compact ? 'space-y-2' : 'space-y-6')}>
			{props.nodes.map((node, index) => <CurriculumNode key={node.path} node={node} index={index + 1} numbers={numbers} {...props} />)}
		</ul>
	);
}

function CurriculumNode({ node, index, moduleTitle, ...props }: NodeProps & { node: CatalogNode; index?: number; moduleTitle?: string }) {
	const [open, setOpen] = useState(() => contains(node, props.activePath));
	const [seen, setSeen] = useState(props.signal?.n ?? 0);
	if (props.signal && props.signal.n !== seen) {
		setSeen(props.signal.n);
		setOpen(props.signal.open);
	}
	// The open lesson can arrive after the first render, or change to one in another module:
	// open the module that holds it.
	const [seenActive, setSeenActive] = useState(props.activePath);
	if (props.activePath !== seenActive) {
		setSeenActive(props.activePath);
		if (contains(node, props.activePath)) setOpen(true);
	}
	if (node.role === 'lesson') return <LessonRow node={node} moduleTitle={moduleTitle} {...props} />;
	const { done, total } = courseProgress(node, props.completed);
	const current = contains(node, props.activePath);
	return (
		<li>
			<details open={open} onToggle={(event) => setOpen(event.currentTarget.open)} className="group">
				<summary className={cn('flex min-h-11 cursor-pointer list-none items-center gap-3 border-b border-[var(--study-rule)] py-2 transition-colors duration-150 hover:text-foreground [&::-webkit-details-marker]:hidden', current && 'text-foreground')}>
					{index !== undefined && <span className="w-6 shrink-0 text-right text-[12.5px] tabular-nums text-muted-foreground" aria-hidden="true">{index}</span>}
					<span className={cn('min-w-0 flex-1 font-semibold', props.compact ? 'text-sm leading-snug' : 'text-[15px] tracking-tight')}>{node.title}{current && props.compact && <span className="sr-only">{tr(' (current module)', ' (módulo atual)')}</span>}</span>
					{total > 0 && <span className="shrink-0 text-[12.5px] tabular-nums text-muted-foreground">{tr(`${done} of ${total} done`, `${done} de ${total} concluídas`)}</span>}
					<ChevronDown className={chevron} aria-hidden="true" />
				</summary>
				{node.children.length > 0 && (
					<ul>
						{node.children.map((child) => (
							<CurriculumNode key={child.path} node={child} moduleTitle={node.children.length === 1 ? node.title : undefined} {...props} />
						))}
					</ul>
				)}
			</details>
		</li>
	);
}

function LessonRow({ node, moduleTitle, courseId, completed, favorites, activePath, compact, resumeSeconds, numbers }: NodeProps & { node: CatalogNode; moduleTitle?: string }) {
	const setCompleted = useSetLessonCompleted();
	const toggleFavorite = useToggleLessonFavorite();
	const mediaId = lessonMediaId(node);
	const missing = isLessonMissing(node);
	const isDone = Boolean(mediaId && completed.has(mediaId));
	const isFavorite = Boolean(mediaId && favorites.has(mediaId));
	const active = activePath === node.path;
	// Only a lesson still to study carries the amber marker; a finished one never does.
	const current = active && !isDone;
	const label = /^(lesson|aula)(\s+\d+)?$/i.test(node.title.trim()) && moduleTitle ? moduleTitle : node.title;
	const singleInModule = !compact && moduleTitle === label;
	const row = useRef<HTMLLIElement>(null);
	// In the player sidebar, bring the open lesson into view inside the panel (never the page).
	useEffect(() => {
		const element = row.current;
		const box = compact && active ? element?.closest('aside') : null;
		if (!element || !box || box.scrollHeight <= box.clientHeight) return;
		box.scrollTop += element.getBoundingClientRect().top - box.getBoundingClientRect().top - box.clientHeight / 2 + element.clientHeight / 2;
	}, [active, compact]);

	// Solid means completed; an outline marks the next or open lesson without implying completion.
	const mark = isDone ? (
		<span className="grid size-3.5 place-items-center rounded-full bg-primary"><Check className="size-2.5 text-primary-foreground" strokeWidth={3} aria-hidden="true" /></span>
	) : current ? (
		<span className="size-3.5 rounded-full border-2 border-primary" />
	) : (
		<span className="size-3.5 rounded-full border-[1.5px] border-border" />
	);
	const status = isDone ? tr('Done', 'Concluída') : current && !missing
		? resumeSeconds ? tr(`Resume at ${formatDuration(resumeSeconds)}`, `Retomar em ${formatDuration(resumeSeconds)}`)
			: compact ? null : tr('Up next', 'A seguir')
		: null;

	return (
		<li ref={row} className="relative flex min-h-11 items-center gap-1 border-b border-[var(--study-rule)] pr-1 transition-colors duration-150">
			{current && <span aria-hidden="true" className="absolute inset-y-2 left-0 w-[3px] rounded-full bg-primary" />}
			<span className="w-7 shrink-0 text-right text-[12.5px] tabular-nums text-muted-foreground" aria-hidden="true">{numbers.get(node.path)}</span>
			{studySyncSupported && mediaId && !missing ? (
				<Button variant="ghost" size="icon" className="shrink-0" aria-pressed={isDone} disabled={setCompleted.isPending} onClick={() => setCompleted.mutate({ mediaId, completed: !isDone })}
					aria-label={isDone ? tr(`Mark “${node.title}” as not completed`, `Marcar “${node.title}” como não concluída`) : tr(`Mark “${node.title}” as completed`, `Marcar “${node.title}” como concluída`)}>
					{mark}
				</Button>
			) : (
				<span className="grid size-11 shrink-0 place-items-center" aria-hidden="true">{missing ? <TriangleAlert className="size-4 text-muted-foreground" /> : mark}</span>
			)}
			{missing ? (
				<div className="flex min-h-11 min-w-0 flex-1 items-center gap-3 py-2 text-muted-foreground">
					<span className="min-w-0 flex-1">{label}</span>
					<span className="shrink-0 text-xs">{tr('Missing media', 'Mídia ausente')}</span>
				</div>
			) : (
				<>
					<Link to={href('course', courseId, node.path).replace(/^#/, '')} aria-current={compact && active ? 'page' : undefined} className={cn('flex min-h-11 min-w-0 flex-1 items-center rounded-md py-2 text-sm hover:underline', isDone ? 'text-muted-foreground' : current ? 'font-semibold text-foreground' : 'text-foreground/85')}>
						{singleInModule && <span className="sr-only">{tr('Open lesson ', 'Abrir aula ')}</span>}
						<span className="min-w-0 flex-1 [overflow-wrap:anywhere]">{label}</span>
					</Link>
					{status && <span className={cn('shrink-0 text-[12.5px] tabular-nums', isDone ? 'text-muted-foreground' : 'text-primary')}>{status}</span>}
				</>
			)}
			{studySyncSupported && mediaId && !missing && (
				<Button variant="ghost" size="icon" className="shrink-0" aria-pressed={isFavorite} disabled={toggleFavorite.isPending} onClick={() => toggleFavorite.mutate({ mediaId, favorite: !isFavorite })}
					aria-label={isFavorite ? tr(`Remove “${node.title}” from favorites`, `Remover “${node.title}” dos favoritos`) : tr(`Add “${node.title}” to favorites`, `Adicionar “${node.title}” aos favoritos`)}>
					<Star className={cn(isFavorite ? 'fill-primary text-primary' : 'text-muted-foreground')} />
				</Button>
			)}
		</li>
	);
}
