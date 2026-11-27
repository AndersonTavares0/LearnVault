import { History } from 'lucide-react';
import { useMemo } from 'react';
import { Link } from 'react-router';
import { QueryFailure } from '@/components/query-failure';
import { PageHeader } from '@/app/shell';
import { routerPath } from '@/components/course-card';
import { buttonVariants } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useCatalog, useRecent } from '@/app/data';
import { courseProgress, displayLessonTitle, formatLocationContext } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import { useLocale } from '@/hooks/use-locale';
import { formatDuration } from '@/lib/player';
import { findCourse, findLessonLocation, href } from '@/lib/router';

/**
 * Every lesson the learner watched, most recent first. It is derived from the same study
 * state that drives the resume card, so completing a lesson or moving its position updates
 * this page without a second history store.
 */
export function HistoryPage() {
	useLocale();
	const catalog = useCatalog();
	const recent = useRecent();
	const completed = useMemo(() => new Set((recent.data ?? []).filter((item) => item.completed).map((item) => item.mediaId)), [recent.data]);

	const rows = useMemo(() => {
		const seen = new Set<string>();
		return (recent.data ?? []).flatMap((item) => {
			const location = findLessonLocation(catalog.data ?? null, item.mediaId);
			if (!location) return [];
			const key = `${location.courseId}/${location.lesson.path}`;
			if (seen.has(key)) return [];
			seen.add(key);
			const course = findCourse(catalog.data ?? null, location.courseId);
			const progress = course ? courseProgress(course, completed) : { done: 0, total: 0 };
			return [{ item, location, progress }];
		});
	}, [catalog.data, recent.data, completed]);

	if (catalog.isPending || recent.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading your history…', 'Carregando seu histórico…')}</p>;
	if (catalog.isError || recent.isError) {
		const failed = catalog.isError ? catalog : recent;
		return <QueryFailure message={tr('Could not load your history.', 'Não foi possível carregar seu histórico.')} retry={() => void failed.refetch()} />;
	}

	return (
		<div className="space-y-8">
			<PageHeader icon={History} title={tr('Watch history', 'Histórico de aulas')} description={tr('The lessons you opened, most recent first. Each link resumes where you stopped.', 'As aulas que você abriu, da mais recente para a mais antiga. Cada link retoma de onde você parou.')} />
			{rows.length === 0 ? (() => {
				const firstCourse = catalog.data?.nodes.find((node) => node.role === 'course');
				return (
					<div className="study-empty">
						<span aria-hidden="true" className="mb-1 inline-flex size-11 items-center justify-center rounded-xl border border-primary/20 bg-brand-soft text-brand-ink"><History className="size-5" /></span>
						<h2 className="text-lg font-semibold tracking-tight">{tr('No lessons watched yet', 'Nenhuma aula assistida ainda')}</h2>
						<p className="text-sm text-muted-foreground">{tr('Open a course and the lessons you study will appear here.', 'Abra um curso e as aulas que você estudar aparecerão aqui.')}</p>
						<div className="flex flex-wrap items-center justify-start gap-2 pt-2">
							{firstCourse?.courseId && <Link to={`/course/${encodeURIComponent(firstCourse.courseId)}`} className={buttonVariants({ variant: 'default' })}>{tr(`Open ${firstCourse.title}`, `Abrir ${firstCourse.title}`)}</Link>}
							<Link to="/" className={buttonVariants({ variant: firstCourse?.courseId ? 'outline' : 'default' })}>{tr('Browse your library', 'Ver sua biblioteca')}</Link>
						</div>
					</div>
				);
			})() : (
				<ol aria-label={tr('Watched lessons', 'Aulas assistidas')} className="divide-y divide-[var(--study-rule)]">
					{rows.map(({ item, location, progress }) => {
						const watchedAt = item.updatedAt ? new Date(item.updatedAt * 1000) : null;
						const resumeAt = !item.completed && item.positionSeconds > 0 ? item.positionSeconds : undefined;
						return (
							<li key={`${location.courseId}/${location.lesson.path}`} className="flex flex-wrap items-center gap-x-6 gap-y-3 py-5">
								<div className="min-w-0 flex-1">
									<p className="text-xs font-medium text-muted-foreground">{formatLocationContext(location.courseTitle, location.moduleTitle)}</p>
									<h2 className="mt-1 text-base font-semibold">
										<Link to={routerPath(href('course', location.courseId, location.lesson.path, resumeAt))} className="underline-offset-4 hover:underline focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{displayLessonTitle(location.lesson.title, location.moduleTitle)}</Link>
									</h2>
									<p className="mt-1 flex flex-wrap gap-x-3 text-sm text-muted-foreground">
										<span>{item.completed ? tr('Completed', 'Concluída') : item.positionSeconds > 0 ? tr(`Stopped at ${formatDuration(item.positionSeconds)}`, `Parou em ${formatDuration(item.positionSeconds)}`) : tr('Opened, not started', 'Aberta, não iniciada')}</span>
										{watchedAt && <time dateTime={watchedAt.toISOString()}>{tr('Last watched', 'Última vez')} {watchedAt.toLocaleDateString(getLocale(), { dateStyle: 'medium' })}</time>}
									</p>
								</div>
								{progress.total > 0 && (
									<div className="w-full sm:w-44">
										<div className="flex justify-between text-xs text-muted-foreground"><span>{tr('Course progress', 'Progresso do curso')}</span><span className="tabular-nums">{progress.done}/{progress.total}</span></div>
										<div role="progressbar" aria-label={tr(`Course progress: ${progress.done} of ${progress.total} lessons`, `Progresso do curso: ${progress.done} de ${progress.total} aulas`)} aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.done} className="mt-1.5 h-1 overflow-hidden rounded-full bg-muted">
											<div className="h-full rounded-full bg-primary" style={{ width: `${(progress.done / progress.total) * 100}%` }} />
										</div>
									</div>
								)}
								<Link to={routerPath(href('course', location.courseId, location.lesson.path, resumeAt))} className={cn(buttonVariants({ variant: 'outline' }), 'min-h-11 px-5')}>{item.completed ? tr('Review', 'Revisar') : tr('Resume', 'Retomar')}</Link>
							</li>
						);
					})}
				</ol>
			)}
		</div>
	);
}
