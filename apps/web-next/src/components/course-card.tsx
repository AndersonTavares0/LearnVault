import { Star } from 'lucide-react';
import { useId } from 'react';
import { Link } from 'react-router';
import { CourseCover } from '@/components/course-cover';
import { Button } from '@/components/ui/button';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import type { CourseEntry } from '@/lib/library';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import { cn } from '@/lib/utils';
import { useToggleCourseFavorite } from '@/app/data';

/** Router paths drop the leading `#` of the canonical hash links built by `href()`. */
export const routerPath = (hash: string) => hash.replace(/^#/, '');

export function ProgressBar({ value, label, className }: { value: number; label: string; className?: string }) {
	const id = useId();
	const percent = Math.round(Math.min(1, Math.max(0, value)) * 100);
	return (
		<div className={cn('space-y-1', className)}>
			<div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-labelledby={id} className="h-1.5 overflow-hidden rounded-full bg-muted">
				<div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} />
			</div>
			<p id={id} className="text-sm text-muted-foreground">{label}</p>
		</div>
	);
}

/** "42% complete · 3 lessons left", or its not-started and completed forms. */
export function progressLabel(entry: CourseEntry): string {
	const percent = entry.total ? Math.round((entry.done / entry.total) * 100) : 0;
	const left = formatCount(entry.total - entry.done, tr('lesson left', 'aula restante'), tr('lessons left', 'aulas restantes'));
	if (entry.status === 'completed') return tr('Completed', 'Concluído');
	if (entry.status === 'not_started') return formatCount(entry.total, tr('lesson', 'aula'), tr('lessons', 'aulas'));
	return `${tr(`${percent}% complete`, `${percent}% concluído`)} · ${left}`;
}

type Props = { entry: CourseEntry; profile: string; variant?: 'grid' | 'shelf' | 'list' };

/**
 * One course. The cover is the only heavy object; everything else is plain text, so a row
 * of them reads like books on a shelf instead of a wall of boxes. A generated cover already
 * carries the title, so the heading beside it collapses to nothing visible but stays in the
 * accessibility tree and keeps the whole-card link (`sr-only` would clip it); the card shows focus.
 */
export function CourseCard({ entry, profile, variant = 'grid' }: Props) {
	const toggle = useToggleCourseFavorite();
	const listed = variant === 'list';
	return (
		<li className={cn('group/card relative rounded-lg has-focus-visible:ring-3 has-focus-visible:ring-ring [content-visibility:auto]', variant === 'shelf' && 'w-64 shrink-0 snap-start sm:w-72', listed && 'flex items-center gap-4 py-3')}>
			<div className={cn('relative', listed && 'w-36 shrink-0')}>
				<CourseCover course={entry.course} profile={profile} />
				{studySyncSupported && (
					<div className="absolute top-1 right-1 z-10">
						<Button variant="ghost" size="icon" aria-pressed={entry.favorite} disabled={toggle.isPending} onClick={() => toggle.mutate({ courseId: entry.courseId, favorite: !entry.favorite })}
							aria-label={entry.favorite ? tr(`Remove ${entry.course.title} from favorites`, `Remover ${entry.course.title} dos favoritos`) : tr(`Add ${entry.course.title} to favorites`, `Adicionar ${entry.course.title} aos favoritos`)}
							className="rounded-full bg-black/35 text-white hover:bg-black/55 hover:text-white aria-pressed:bg-black/55">
							<Star className={cn(entry.favorite && 'fill-white')} />
						</Button>
					</div>
				)}
			</div>
			<div className={cn('min-w-0 space-y-1.5', !listed && 'mt-3', listed && 'flex-1')}>
				<h3 className="text-base leading-snug font-semibold text-balance group-has-[[data-titled]]/card:m-0 group-has-[[data-titled]]/card:h-0 group-has-[[data-titled]]/card:opacity-0">
					{/* The link covers the whole card; the favorite button sits above it. */}
					<Link to={routerPath(href('course', entry.courseId))} className="line-clamp-2 after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
				</h3>
				{entry.total > 0 && <ProgressBar value={entry.done / Math.max(1, entry.total)} label={progressLabel(entry)} />}
				{listed && entry.next && entry.status === 'in_progress' && <p className="truncate text-sm">{tr('Next:', 'Próxima:')} {entry.next.title}</p>}
			</div>
		</li>
	);
}

/** A flat row for "Also in progress": a small cover, the course and where to pick it up. */
export function CourseRow({ entry, profile }: { entry: CourseEntry; profile: string }) {
	return (
		<li className="relative flex items-center gap-4 border-t py-3 hover:bg-accent/40">
			<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="w-24 shrink-0" />
			<div className="min-w-0 flex-1">
				<h3 className="truncate font-semibold">
					<Link to={routerPath(href('course', entry.courseId))} className="after:absolute after:inset-0 after:content-['']">{entry.course.title}</Link>
				</h3>
				<p className="truncate text-sm text-muted-foreground">{entry.status === 'in_progress' && entry.next ? `${tr('Next:', 'Próxima:')} ${entry.next.title}` : progressLabel(entry)}</p>
			</div>
			<span className="text-sm font-semibold text-primary">{tr('Resume', 'Retomar')}</span>
		</li>
	);
}
