import { Star } from 'lucide-react';
import { useId } from 'react';
import { Link } from 'react-router';
import { CourseCover } from '@/components/course-cover';
import { Button } from '@/components/ui/button';
import { formatCount } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
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
			<p id={id} className="meta text-sm text-muted-foreground">{label}</p>
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

type Props = { entry: CourseEntry; profile: string; variant?: 'grid' | 'shelf' | 'list' | 'compact' };

/**
 * One course. The cover is the only heavy object; everything else is plain text, so a row
 * of them reads like books on a shelf instead of a wall of boxes. A generated cover already
 * carries the title, so the heading beside it collapses to nothing visible but stays in the
 * accessibility tree and keeps the whole-card link (`sr-only` would clip it); the card shows focus.
 */
export function CourseCard({ entry, profile, variant = 'grid' }: Props) {
	const toggle = useToggleCourseFavorite();
	const listed = variant === 'list';
	const lastStudied = entry.lastStudiedAt ? new Date(entry.lastStudiedAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' }) : '–';
	if (variant === 'compact') {
		const percent = entry.total ? Math.round((entry.done / entry.total) * 100) : 0;
		return (
			<li className="relative flex items-center gap-3 py-2.5">
				<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="w-10 shrink-0" />
				<div className="min-w-0 flex-1">
					<h3 className="truncate font-heading text-sm font-semibold">
						<Link to={routerPath(href('course', entry.courseId))} className="after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
					</h3>
					<p className="truncate text-xs text-muted-foreground">{progressLabel(entry)}</p>
				</div>
				<div aria-hidden="true" className="hidden w-32 shrink-0 lg:block">
					<div className="h-1.5 overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} /></div>
				</div>
				<p className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground lg:block">{lastStudied}</p>
				<span className="shrink-0 text-xs font-semibold text-primary">{tr('Resume', 'Retomar')}</span>
			</li>
		);
	}
	return (
		<li className={cn('group/card relative [content-visibility:auto] has-focus-visible:ring-3 has-focus-visible:ring-ring', !listed && 'overflow-hidden rounded-[1.75rem] border bg-card transition-[border-color,box-shadow] hover:border-[color:var(--border-strong)] hover:shadow-md', variant === 'shelf' && 'w-64 shrink-0 snap-start sm:w-72', listed && 'flex items-center gap-4 rounded-2xl py-2.5')}>
			<div className={cn('relative', listed && 'w-20 shrink-0')}>
				<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact={listed} />
				{studySyncSupported && !listed && (
					<div className="absolute top-1 right-1 z-10">
						<Button variant="ghost" size="icon" aria-pressed={entry.favorite} disabled={toggle.isPending} onClick={() => toggle.mutate({ courseId: entry.courseId, favorite: !entry.favorite })}
							aria-label={entry.favorite ? tr(`Remove ${entry.course.title} from favorites`, `Remover ${entry.course.title} dos favoritos`) : tr(`Add ${entry.course.title} to favorites`, `Adicionar ${entry.course.title} aos favoritos`)}
							className="rounded-full bg-black/35 text-white hover:bg-black/55 hover:text-white aria-pressed:bg-black/55">
							<Star className={cn(entry.favorite && 'fill-white')} />
						</Button>
					</div>
				)}
			</div>
			<div className={cn('min-w-0 space-y-2', !listed && 'p-5', listed && 'flex-1')}>
				<h3 className={cn('card-heading font-heading leading-snug font-semibold text-balance', listed ? 'text-base' : 'text-lg')}>
					{/* The link covers the whole card; the favorite button sits above it. */}
					<Link to={routerPath(href('course', entry.courseId))} className="line-clamp-2 after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
				</h3>
				{listed ? (
					<div className="flex flex-wrap items-center gap-x-4 gap-y-1">
						{entry.total > 0 && <div className="w-52 max-w-full"><ProgressBar value={entry.done / Math.max(1, entry.total)} label={progressLabel(entry)} /></div>}
						<span className="text-sm text-muted-foreground">{entry.lastStudiedAt ? tr(`Studied ${lastStudied}`, `Estudado ${lastStudied}`) : tr('Not studied', 'Não estudado')}</span>
						<span className="ml-auto text-sm font-semibold text-primary">{tr('Resume', 'Retomar')}</span>
					</div>
				) : entry.total > 0 ? (
					<ProgressBar value={entry.done / Math.max(1, entry.total)} label={progressLabel(entry)} />
				) : null}
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
