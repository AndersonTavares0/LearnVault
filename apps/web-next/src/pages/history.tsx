import { History } from 'lucide-react';
import { useMemo } from 'react';
import { Link } from 'react-router';
import { PageHeader } from '@/app/shell';
import { routerPath } from '@/components/course-card';
import { Panel } from '@/components/panel';
import { useCatalog, useRecent } from '@/app/data';
import { courseProgress, formatLocationContext } from '@/lib/catalog';
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
	if (catalog.isError) return <p role="alert" className="text-destructive">{catalog.error.message}</p>;

	return (
		<div className="space-y-8">
			<PageHeader icon={History} title={tr('Watch history', 'Histórico de aulas')} description={tr('The lessons you opened, most recent first. Each link resumes where you stopped.', 'As aulas que você abriu, da mais recente para a mais antiga. Cada link retoma de onde você parou.')} />
			{rows.length === 0 ? (
				<Panel className="mx-auto max-w-xl space-y-4 text-center">
					<History className="mx-auto size-8 text-muted-foreground" aria-hidden="true" />
					<p className="font-heading text-2xl font-semibold">{tr('No lessons watched yet', 'Nenhuma aula assistida ainda')}</p>
					<p className="text-muted-foreground">{tr('Open a course and the lessons you study will appear here.', 'Abra um curso e as aulas que você estudar aparecerão aqui.')}</p>
					<Link to="/" className="inline-flex min-h-11 items-center rounded-full bg-primary px-6 font-semibold text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('Browse your library', 'Ver sua biblioteca')}</Link>
				</Panel>
			) : (
				<Panel className="p-0 sm:p-0">
					<ol aria-label={tr('Watched lessons', 'Aulas assistidas')} className="divide-y">
						{rows.map(({ item, location, progress }) => {
							const watchedAt = item.updatedAt ? new Date(item.updatedAt * 1000) : null;
							const resumeAt = !item.completed && item.positionSeconds > 0 ? item.positionSeconds : undefined;
							return (
								<li key={`${location.courseId}/${location.lesson.path}`} className="flex flex-wrap items-center gap-x-6 gap-y-3 p-5 sm:p-6">
									<div className="min-w-0 flex-1">
										<p className="text-xs font-semibold tracking-[0.12em] text-muted-foreground uppercase">{formatLocationContext(location.courseTitle, location.moduleTitle)}</p>
										<h2 className="mt-1 font-heading text-xl font-semibold">
											<Link to={routerPath(href('course', location.courseId, location.lesson.path, resumeAt))} className="hover:text-primary focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{location.lesson.title}</Link>
										</h2>
										<p className="mt-1 text-sm text-muted-foreground">
											{item.completed ? tr('Completed', 'Concluída') : item.positionSeconds > 0 ? tr(`Stopped at ${formatDuration(item.positionSeconds)}`, `Parou em ${formatDuration(item.positionSeconds)}`) : tr('Opened, not started', 'Aberta, não iniciada')}
											{watchedAt && <> · <time dateTime={watchedAt.toISOString()}>{tr('Last watched', 'Última vez')} {watchedAt.toLocaleDateString(getLocale(), { dateStyle: 'medium' })}</time></>}
										</p>
									</div>
									{progress.total > 0 && (
										<div className="w-full sm:w-44">
											<div className="flex justify-between text-xs text-muted-foreground"><span>{tr('Course progress', 'Progresso do curso')}</span><span className="tabular-nums">{progress.done}/{progress.total}</span></div>
											<div role="progressbar" aria-label={tr(`Course progress: ${progress.done} of ${progress.total} lessons`, `Progresso do curso: ${progress.done} de ${progress.total} aulas`)} aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.done} className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-border">
												<div className="h-full rounded-full bg-primary" style={{ width: `${(progress.done / progress.total) * 100}%` }} />
											</div>
										</div>
									)}
									<Link to={routerPath(href('course', location.courseId, location.lesson.path, resumeAt))} className="inline-flex min-h-11 items-center rounded-full border bg-card px-6 text-sm font-medium hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{item.completed ? tr('Review', 'Revisar') : tr('Resume', 'Retomar')}</Link>
								</li>
							);
						})}
					</ol>
				</Panel>
			)}
		</div>
	);
}
