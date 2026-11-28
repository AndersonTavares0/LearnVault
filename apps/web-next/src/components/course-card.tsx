import { Star } from 'lucide-react';
import { useId, type ReactNode } from 'react';
import { Link } from 'react-router';
import { CourseCover } from '@/components/course-cover';
import { CourseManagement } from '@/components/course-management';
import { LessonSegments } from '@/components/lesson-segments';
import { Button } from '@/components/ui/button';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { formatCount, isLessonMissing } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import type { CourseEntry } from '@/lib/library';
import type { CatalogNode } from '@/lib/transport';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import { cn } from '@/lib/utils';
import { useToggleCourseFavorite } from '@/app/data';

/** Router paths drop the leading `#` of the canonical hash links built by `href()`. */
export const routerPath = (hash: string) => hash.replace(/^#/, '');

/** Lesson paths in course order, the same lessons `courseProgress` counts; the segments follow this order. */
export function lessonPaths(node: CatalogNode, out: string[] = []): string[] {
	if (node.role === 'lesson' && !isLessonMissing(node)) out.push(node.path);
	for (const child of node.children) lessonPaths(child, out);
	return out;
}

export function ProgressBar({ value, label, action, className }: { value: number; label: string; action?: ReactNode; className?: string }) {
	const id = useId();
	const percent = Math.round(Math.min(1, Math.max(0, value)) * 100);
	return (
		<div className={cn('space-y-1.5', className)}>
			<div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-labelledby={id} className="h-1 overflow-hidden rounded-full bg-muted">
				<div className="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: `${percent}%` }} />
			</div>
			<div className="flex items-baseline justify-between gap-2">
				<p id={id} className="meta min-w-0 truncate text-xs text-muted-foreground">{label}</p>
				{action}
			</div>
		</div>
	);
}

/** "42% complete · 3 lessons left", or its not-started and completed forms. */
export function progressLabel(entry: CourseEntry): string {
	if (entry.total === 0) return tr('No lessons found', 'Nenhuma aula encontrada');
	const percent = entry.total ? Math.round((entry.done / entry.total) * 100) : 0;
	const left = formatCount(entry.total - entry.done, tr('lesson left', 'aula restante'), tr('lessons left', 'aulas restantes'));
	if (entry.status === 'completed') return tr('Completed', 'Concluído');
	if (entry.status === 'not_started') return formatCount(entry.total, tr('lesson', 'aula'), tr('lessons', 'aulas'));
	return `${tr(`${percent}% complete`, `${percent}% concluído`)} · ${left}`;
}

type Props = { entry: CourseEntry; profile: string; variant?: 'grid' | 'shelf' | 'list' | 'compact'; canManage?: boolean };

/**
 * One course. The cover is the only heavy object; everything else is plain text, so a row
 * of them reads like books on a shelf instead of a wall of boxes. A generated cover already
 * carries the title, so the heading beside it collapses to nothing visible but stays in the
 * accessibility tree and keeps the whole-card link (`sr-only` would clip it); the card shows focus.
 */
export function CourseCard({ entry, profile, variant = 'grid', canManage = false }: Props) {
	const look = useStore(appearanceStore);
	const toggle = useToggleCourseFavorite();
	const favoriteError = toggle.isError && (
		<p role="alert" className="mt-2 text-xs text-destructive">
			{tr('Could not update favorites. Try again.', 'Não foi possível atualizar os favoritos. Tente novamente.')}
		</p>
	);
	// Cover, lesson order and registry removal only exist for local courses; a remote course is managed from Settings.
	const manageable = canManage && !entry.course.remote;
	const listed = variant === 'list';
	const lastStudied = entry.lastStudiedAt ? new Date(entry.lastStudiedAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' }) : '–';
	if (variant === 'compact') {
		const percent = entry.total ? Math.round((entry.done / entry.total) * 100) : 0;
		return (
			<li data-row className="group/row relative -mx-2.5 flex items-center gap-3 rounded-xl px-2.5 py-2.5 transition-colors duration-150 hover:bg-panel/45">
				<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="w-10 shrink-0 rounded-md ring-1 ring-border/75" />
				<div className="min-w-0 flex-1">
					<h3 className="truncate text-sm font-semibold">
						<Link to={routerPath(href('course', entry.courseId))} className="after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
					</h3>
					<p className="truncate text-xs text-muted-foreground">{progressLabel(entry)}</p>
				</div>
				<div aria-hidden="true" className="hidden w-32 shrink-0 lg:block">
					<div className="h-1.5 overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} /></div>
				</div>
				<p className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground lg:block">{lastStudied}</p>
				{entry.status === 'in_progress' && entry.next ? (
					<Link to={routerPath(href('course', entry.courseId, entry.next.path))} className="relative z-10 shrink-0 text-xs font-semibold text-primary hover:underline" aria-label={tr(`Resume ${entry.course.title}: ${entry.next.title}`, `Retomar ${entry.course.title}: ${entry.next.title}`)}>{tr('Resume', 'Retomar')}</Link>
				) : (
					<span className="shrink-0 text-xs font-semibold text-primary">{tr('Open', 'Abrir')}</span>
				)}
				{manageable && <div className="relative z-10"><CourseManagement course={entry.course} courseId={entry.courseId} lessons={entry.total} compact /></div>}
			</li>
		);
	}
	if (listed) {
		return (
			<li data-row className="group/row relative -mx-3 flex items-center gap-4 rounded-xl border-b px-3 py-3 transition-colors duration-150 first:border-t hover:bg-panel/45 has-focus-visible:ring-3 has-focus-visible:ring-ring">
				<div className="w-11 shrink-0"><CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="aspect-square rounded-md ring-1 ring-border/75" /></div>
				<div className="min-w-0 flex-1">
					<h3 className="text-base font-semibold">
						<Link to={routerPath(href('course', entry.courseId))} title={entry.course.title} className="line-clamp-2 [overflow-wrap:anywhere] after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
					</h3>
					<p className="truncate text-sm text-muted-foreground">{entry.next && entry.status === 'in_progress' ? `${tr('Next:', 'Próxima:')} ${entry.next.title}` : entry.total === 0 ? tr('No lessons found', 'Sem aulas') : entry.status === 'not_started' ? tr('Not started', 'Não iniciado') : progressLabel(entry)}</p>
				</div>
				<div className="hidden w-56 shrink-0 sm:block">{entry.done > 0 ? <ProgressBar value={entry.done / Math.max(1, entry.total)} label={progressLabel(entry)} /> : <p className="text-sm text-muted-foreground">{progressLabel(entry)}</p>}</div>
				<span className="hidden w-32 shrink-0 text-sm text-muted-foreground md:block">{entry.lastStudiedAt ? tr(`Studied ${lastStudied}`, `Estudado ${lastStudied}`) : tr('Not studied', 'Não estudado')}</span>
				{entry.status === 'in_progress' && entry.next ? (
					<Link to={routerPath(href('course', entry.courseId, entry.next.path))} className="relative z-10 inline-flex min-h-11 w-16 shrink-0 items-center justify-end text-sm font-semibold text-primary hover:underline focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none" aria-label={tr(`Resume ${entry.course.title}: ${entry.next.title}`, `Retomar ${entry.course.title}: ${entry.next.title}`)}>{tr('Resume', 'Retomar')}</Link>
				) : <span className="w-16 shrink-0 text-right text-sm font-semibold text-primary">{tr('Open', 'Abrir')}</span>}
				{manageable && <div className="relative z-10"><CourseManagement course={entry.course} courseId={entry.courseId} lessons={entry.total} compact /></div>}
			</li>
		);
	}
	if (variant === 'grid') {
		const current = entry.status === 'in_progress' && entry.next ? lessonPaths(entry.course).indexOf(entry.next.path) : -1;
		const doneLabel = entry.done > 0 ? tr(`${entry.done} done`, `${entry.done} ${entry.done === 1 ? 'concluída' : 'concluídas'}`) : entry.status === 'in_progress' ? tr('Started', 'Iniciado') : tr('Not started', 'Não iniciado');
		return (
			<li className="group/card relative grid min-w-0 grid-cols-[4.25rem_minmax(0,1fr)] items-start gap-3.5 border-t border-[var(--study-rule)] py-3.5 sm:grid-cols-[4.5rem_minmax(0,1fr)]">
				<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="aspect-square w-full rounded-[var(--radius)] border border-[var(--study-rule)] ring-0" />
				<div className="flex min-h-[4.5rem] min-w-0 flex-col justify-between gap-2">
					<div className="flex min-h-[2lh] items-start gap-1 text-base leading-snug">
						<h3 className="line-clamp-2 min-w-0 flex-1 font-semibold tracking-[-0.01em] [overflow-wrap:anywhere]">
							<Link to={routerPath(href('course', entry.courseId))} title={entry.course.title} className="after:absolute after:inset-0 after:content-[''] hover:underline focus-visible:underline">{entry.course.title}</Link>
						</h3>
						{(studySyncSupported || manageable) && (
							<div className="relative z-10 -mt-1.5 -mr-1.5 flex shrink-0 items-center text-muted-foreground">
								{studySyncSupported && (
									<Button variant="ghost" size="icon" aria-pressed={entry.favorite} disabled={toggle.isPending} onClick={() => toggle.mutate({ courseId: entry.courseId, favorite: !entry.favorite })}
										aria-label={entry.favorite ? tr(`Remove ${entry.course.title} from favorites`, `Remover ${entry.course.title} dos favoritos`) : tr(`Add ${entry.course.title} to favorites`, `Adicionar ${entry.course.title} aos favoritos`)}
										className="opacity-0 group-focus-within/card:opacity-100 group-hover/card:opacity-100 aria-pressed:text-primary aria-pressed:opacity-100 [@media(hover:none)]:opacity-100">
										<Star className={cn(entry.favorite && 'fill-current')} aria-hidden="true" />
									</Button>
								)}
								{manageable && <CourseManagement course={entry.course} courseId={entry.courseId} lessons={entry.total} compact className="opacity-0 group-focus-within/card:opacity-100 group-hover/card:opacity-100 aria-expanded:opacity-100 [@media(hover:none)]:opacity-100" />}
							</div>
						)}
					</div>
					{entry.total > 0 ? (
						<div className="space-y-1.5">
							<p className="flex flex-wrap items-center gap-x-2.5 gap-y-0.5 text-xs text-muted-foreground tabular-nums">
								<span>{formatCount(entry.total, tr('lesson', 'aula'), tr('lessons', 'aulas'))}</span>
								<span aria-hidden="true">·</span>
								<span className={entry.status === 'completed' ? 'font-medium text-primary' : undefined}>{doneLabel}</span>
							</p>
							{(look.showProgress || entry.done > 0) && (
								<LessonSegments done={entry.done} total={entry.total} current={current >= 0 ? current : undefined} />
							)}
						</div>
					) : (
						<p className="text-[12.5px] text-muted-foreground">{tr('No lessons found', 'Sem aulas')}</p>
					)}
					{favoriteError}
				</div>
			</li>
		);
	}
	const resumeClass = 'relative z-10 inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center px-1 text-xs font-semibold text-primary hover:underline focus-visible:outline-2 focus-visible:outline-ring';
	return (
		<li data-card className="group/card study-surface relative flex w-64 shrink-0 snap-start flex-col items-stretch justify-between overflow-hidden p-0 transition-colors duration-150 hover:border-[color:var(--border-strong)] has-focus-visible:ring-2 has-focus-visible:ring-ring">
			<div className="relative overflow-hidden border-b border-[var(--study-rule)]">
				<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} className="aspect-[2.4/1] w-full rounded-none ring-0 [&_img]:transition-transform [&_img]:duration-200 [&_img]:ease-[cubic-bezier(0.23,1,0.32,1)] group-hover/card:[&_img]:scale-[1.03]" />
			</div>
			<div className="flex min-w-0 flex-1 flex-col justify-between gap-2 p-3.5">
				<div className="flex items-center gap-2.5">
					<h3 className="card-heading flex min-h-11 min-w-0 flex-1 items-center text-base leading-snug font-semibold">
						<Link to={routerPath(href('course', entry.courseId))} title={entry.course.title} className="line-clamp-2 [overflow-wrap:anywhere] after:absolute after:inset-0 after:content-[''] hover:underline focus-visible:underline">{entry.course.title}</Link>
					</h3>
					{(studySyncSupported || manageable) && (
						<div className="relative z-10 -mr-1.5 flex shrink-0 items-center text-muted-foreground">
							{studySyncSupported && (
								<Button variant="ghost" size="icon" aria-pressed={entry.favorite} disabled={toggle.isPending} onClick={() => toggle.mutate({ courseId: entry.courseId, favorite: !entry.favorite })}
									aria-label={entry.favorite ? tr(`Remove ${entry.course.title} from favorites`, `Remover ${entry.course.title} dos favoritos`) : tr(`Add ${entry.course.title} to favorites`, `Adicionar ${entry.course.title} aos favoritos`)}
									className="opacity-0 group-focus-within/card:opacity-100 group-hover/card:opacity-100 aria-pressed:text-primary aria-pressed:opacity-100 [@media(hover:none)]:opacity-100">
									<Star className={cn(entry.favorite && 'fill-current')} aria-hidden="true" />
								</Button>
							)}
							{manageable && <CourseManagement course={entry.course} courseId={entry.courseId} lessons={entry.total} compact className="opacity-0 group-focus-within/card:opacity-100 group-hover/card:opacity-100 aria-expanded:opacity-100 [@media(hover:none)]:opacity-100" />}
						</div>
					)}
				</div>
				{entry.total > 0 ? (
					entry.done > 0 && look.showProgress ? (
						<ProgressBar
							value={entry.done / Math.max(1, entry.total)}
							label={progressLabel(entry)}
							action={entry.status === 'in_progress' && entry.next ? (
								<Link to={routerPath(href('course', entry.courseId, entry.next.path))} className={resumeClass} aria-label={tr(`Resume ${entry.course.title}: ${entry.next.title}`, `Retomar ${entry.course.title}: ${entry.next.title}`)}>
									{tr('Resume', 'Retomar')}
								</Link>
							) : undefined}
						/>
					) : (
						<div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
							<span className="truncate">{progressLabel(entry)}</span>
							{entry.done > 0 && entry.status === 'in_progress' && entry.next ? (
								<Link to={routerPath(href('course', entry.courseId, entry.next.path))} className={resumeClass} aria-label={tr(`Resume ${entry.course.title}: ${entry.next.title}`, `Retomar ${entry.course.title}: ${entry.next.title}`)}>
									{tr('Resume', 'Retomar')}
								</Link>
							) : look.showCategories && entry.category ? (
								<span className="max-w-[8.5rem] shrink-0 truncate text-xs font-medium text-muted-foreground">{entry.category}</span>
							) : null}
						</div>
					)
				) : (
					<p className="text-xs text-muted-foreground">{tr('No lessons found', 'Sem aulas')}</p>
				)}
				{favoriteError}
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
