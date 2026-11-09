import { ChevronDown, CircleCheck, Circle, FileText, PlayCircle, Star, TriangleAlert } from 'lucide-react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { countCatalogFiles, courseProgress, formatLessonMeta, isLessonMissing } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { studySyncSupported } from '@/lib/player';
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
};

const lessonMediaId = (node: CatalogNode) => [...node.files, ...node.looseFiles].find((file) => file.kind === 'media' && file.mediaId)?.mediaId ?? null;
const contains = (node: CatalogNode, path: string | null | undefined) => Boolean(path) && (node.path === path || path!.startsWith(`${node.path}/`));

/**
 * Course curriculum: modules are native disclosures (keyboard and screen-reader support
 * for free, #321), lessons show completion, type and favourite. Shared by the course page
 * and the player sidebar.
 */
export function Curriculum(props: Props) {
	return (
		<ul className="divide-y rounded-lg border bg-card">
			{props.nodes.map((node) => <CurriculumNode key={node.path} node={node} {...props} />)}
		</ul>
	);
}

function CurriculumNode({ node, ...props }: Props & { node: CatalogNode }) {
	if (node.role === 'lesson') return <LessonRow node={node} {...props} />;
	const { done, total } = courseProgress(node, props.completed);
	return (
		<li>
			<details open={contains(node, props.activePath) || undefined} className="group">
				<summary className="flex min-h-14 cursor-pointer list-none items-center gap-3 px-4 py-3 hover:bg-muted [&::-webkit-details-marker]:hidden">
					<ChevronDown className="size-4 shrink-0 text-muted-foreground transition-transform group-open:rotate-180" aria-hidden="true" />
					<span className="min-w-0 flex-1 font-medium">{node.title}</span>
					{total > 0 && <span className="shrink-0 text-sm text-muted-foreground tabular-nums">{tr(`${done} of ${total} done`, `${done} de ${total} concluídas`)}</span>}
				</summary>
				{node.children.length > 0 && (
					<ul className="divide-y border-t bg-background/50">
						{node.children.map((child) => <CurriculumNode key={child.path} node={child} {...props} />)}
					</ul>
				)}
			</details>
		</li>
	);
}

function LessonRow({ node, courseId, completed, favorites, activePath }: Props & { node: CatalogNode }) {
	const setCompleted = useSetLessonCompleted();
	const toggleFavorite = useToggleLessonFavorite();
	const counts = countCatalogFiles(node);
	const meta = formatLessonMeta(counts);
	const mediaId = lessonMediaId(node);
	const missing = isLessonMissing(node);
	const isDone = Boolean(mediaId && completed.has(mediaId));
	const isFavorite = Boolean(mediaId && favorites.has(mediaId));
	const active = activePath === node.path;
	const Icon = counts.videos > 0 ? PlayCircle : FileText;

	return (
		<li className={cn('flex items-center gap-1 pr-2', active && 'bg-accent')}>
			{studySyncSupported && mediaId && !missing ? (
				<Button variant="ghost" size="icon" aria-pressed={isDone} disabled={setCompleted.isPending} onClick={() => setCompleted.mutate({ mediaId, completed: !isDone })}
					aria-label={isDone ? tr(`Mark “${node.title}” as not completed`, `Marcar “${node.title}” como não concluída`) : tr(`Mark “${node.title}” as completed`, `Marcar “${node.title}” como concluída`)}>
					{isDone ? <CircleCheck className="text-primary" /> : <Circle className="text-muted-foreground" />}
				</Button>
			) : (
				<span className="size-11 shrink-0" aria-hidden="true" />
			)}
			{missing ? (
				<div className="flex min-h-11 min-w-0 flex-1 items-center gap-3 py-2 text-muted-foreground">
					<TriangleAlert className="size-4 shrink-0" aria-hidden="true" />
					<span className="min-w-0 flex-1">{node.title}</span>
					<span className="shrink-0 text-xs">{tr('Missing media', 'Mídia ausente')}</span>
				</div>
			) : (
				<Link to={href('course', courseId, node.path).replace(/^#/, '')} aria-current={active ? 'page' : undefined} className={cn('flex min-h-11 min-w-0 flex-1 items-center gap-3 rounded-md py-2 hover:underline', active && 'font-medium')}>
					<Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
					<span className="min-w-0 flex-1">{node.title}</span>
					{meta && <span className="shrink-0 text-xs text-muted-foreground">{meta}</span>}
				</Link>
			)}
			{studySyncSupported && mediaId && !missing && (
				<Button variant="ghost" size="icon" aria-pressed={isFavorite} disabled={toggleFavorite.isPending} onClick={() => toggleFavorite.mutate({ mediaId, favorite: !isFavorite })}
					aria-label={isFavorite ? tr(`Remove “${node.title}” from favorites`, `Remover “${node.title}” dos favoritos`) : tr(`Add “${node.title}” to favorites`, `Adicionar “${node.title}” aos favoritos`)}>
					<Star className={cn(isFavorite ? 'fill-primary text-primary' : 'text-muted-foreground')} />
				</Button>
			)}
		</li>
	);
}
