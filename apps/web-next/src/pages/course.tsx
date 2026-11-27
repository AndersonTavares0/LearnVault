import { ArrowUpDown, Download, FileText, Pencil, RotateCcw, Search, Star, Tag } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { safeStorage } from '@/app/preferences';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { CourseCategoryDialog } from '@/components/course-category-dialog';
import { CourseManagement } from '@/components/course-management';
import { CourseCover } from '@/components/course-cover';
import { Curriculum, lessonMediaId } from '@/components/curriculum';
import { LessonSegments, type SegmentState } from '@/components/lesson-segments';
import { Markdown } from '@/components/markdown';
import { Materials } from '@/components/materials';
import { NoteEditor } from '@/components/note-editor';
import { Button, buttonVariants } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useLocale } from '@/hooks/use-locale';
import { usePageTitle } from '@/hooks/use-page-title';
import { useStore } from '@/hooks/use-store';
import { collectMaterials, countCatalogNode, displayLessonTitle, isLessonMissing } from '@/lib/catalog';
import { courseCategoriesStore, resolveCourseCategory } from '@/lib/course-categories';
import { tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { plainTextMarkdown } from '@/lib/markdown';
import { isPdf } from '@/lib/pdf';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findCourse, findLessonLocation, href } from '@/lib/router';
import { normalizeForSearch } from '@/lib/search';
import { transport, type CatalogNode, type FileEntry } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { LessonPage } from './lesson';
import { useCatalog, useFavorites, useNotes, useRecent, useSession, useSetCourseOrder, useSetLessonsCompleted, useToggleCourseFavorite } from '@/app/data';
import { useMutation, useQueryClient } from '@tanstack/react-query';

const path = (hash: string) => hash.replace(/^#/, '');

/** Lessons in course order, however deeply their modules nest. */
const lessonNodes = (node: CatalogNode): CatalogNode[] => (node.role === 'lesson' ? [node] : node.children.flatMap(lessonNodes));

/**
 * PDFs kept loose in the course or in its modules: no lesson holds them, so the course page
 * links each one to the reader. Other loose files stay in the Materials tab as downloads.
 */
function loosePdfs(node: CatalogNode): FileEntry[] {
	const files: FileEntry[] = [];
	const walk = (current: CatalogNode) => {
		for (const file of current.looseFiles) {
			if (isPdf(file) && file.mediaId && file.availability?.trim().toLowerCase() !== 'missing') files.push(file);
		}
		current.children.forEach(walk);
	};
	walk(node);
	return files;
}

export function CourseRoute() {
	useLocale();
	const { courseId = '', '*': lessonPath } = useParams();
	const catalog = useCatalog();
	if (catalog.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading course…', 'Carregando curso…')}</p>;
	const course = findCourse(catalog.data ?? null, courseId);
	if (!course) return <NotFound />;
	if (lessonPath) return <LessonPage course={course} courseId={courseId} lessonPath={lessonPath} />;
	return <CourseOverview course={course} courseId={courseId} />;
}

function NotFound() {
	const title = tr('Course not found', 'Curso não encontrado');
	usePageTitle(title);
	return (
		<div className="space-y-4">
			<h1 className="text-2xl font-semibold">{title}</h1>
			<p className="text-muted-foreground">{tr('It may have been removed from the library or you may not have access to it.', 'Ele pode ter sido removido da biblioteca ou você pode não ter acesso a ele.')}</p>
			<Link to="/" className={buttonVariants({ variant: 'outline' })}>{tr('Back to My learning', 'Voltar para Meu aprendizado')}</Link>
		</div>
	);
}

function CourseOverview({ course, courseId }: { course: CatalogNode; courseId: string }) {
	usePageTitle(course.title);
	const navigate = useNavigate();
	const session = useSession();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const notes = useNotes();
	const toggleFavorite = useToggleCourseFavorite();
	const setCourseOrder = useSetCourseOrder();
	const batchComplete = useSetLessonsCompleted();
	const user = session.data?.user;
	const [signal, setSignal] = useState({ open: false, n: 0 });
	const [categorizing, setCategorizing] = useState(false);
	const [lessonQuery, setLessonQuery] = useState('');
	const [incompleteOnly, setIncompleteOnly] = useState(false);
	const [reorderMode, setReorderMode] = useState(false);
	const [confirmReset, setConfirmReset] = useState(false);
	const customCategories = useStore(courseCategoriesStore);
	const category = resolveCourseCategory(course, customCategories);

	const completed = useMemo(() => new Set((recent.data ?? []).filter((item) => item.completed).map((item) => item.mediaId)), [recent.data]);
	const favoriteMedia = useMemo(() => new Set(favorites.data?.mediaIds ?? []), [favorites.data]);
	const entry = useMemo(
		() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set(favorites.data?.courseIds ?? []) }).entries.find((item) => item.courseId === courseId),
		[catalog.data, recent.data, favorites.data, courseId]
	);
	const lastStudied = (recent.data ?? []).map((item) => ({ item, location: findLessonLocation(catalog.data ?? null, item.mediaId) })).find(({ location }) => location?.courseId === courseId);
	const counts = countCatalogNode(course);
	const materials = collectMaterials(course);
	const documents = loosePdfs(course);
	// Files keep their folder in the path; grouping restores the reading list's own structure.
	const documentGroups = useMemo(() => {
		const groups = new Map<string, FileEntry[]>();
		for (const file of documents) {
			const folder = file.path.includes('/') ? file.path.slice(0, file.path.lastIndexOf('/')) : '';
			const list = groups.get(folder);
			if (list) list.push(file);
			else groups.set(folder, [file]);
		}
		return [...groups];
	}, [documents]);
	// A course made only of PDFs is a reading list, not a broken course: no notice, no empty map,
	// and the documents become the primary content.
	const readingOnly = counts.lessons === 0 && documents.length > 0;
	// A material beside the course (a syllabus, a guide) is normal and already in Materials, so it is no warning.
	const concerns = (course.structure?.concerns ?? []).filter((concern) => concern !== 'loose_materials' && concern !== 'lessons_without_numbering');
	const resumeAt = lastStudied && !lastStudied.item.completed ? lastStudied.item.positionSeconds : null;
	// One segment per playable lesson, in course order. The lesson to study next is the one with a saved
	// position, otherwise the first lesson not yet done after the last one studied. Done lessons never are.
	const lessons = course.children.flatMap(lessonNodes).filter((node) => !isLessonMissing(node));
	const isDone = (node: CatalogNode) => completed.has(lessonMediaId(node) ?? '');
	const doneMediaIds = useMemo(
		() => lessons.map((node) => lessonMediaId(node)).filter((id): id is string => Boolean(id && completed.has(id))),
		[lessons, completed]
	);
	const needle = normalizeForSearch(lessonQuery);
	const filtering = Boolean(needle) || incompleteOnly;
	const filteredNodes = useMemo(
		() => filtering ? filterCurriculum(course.children, needle, incompleteOnly, completed) : course.children,
		[filtering, course.children, needle, incompleteOnly, completed]
	);
	const canReorder = Boolean(user?.isAdmin && !course.remote);
	// Untouched courses (0% done and never opened) stay calm and unhighlighted until study begins.
	const hasStarted = Boolean(lastStudied) || lessons.some(isDone);
	const resumeLesson = resumeAt ? lastStudied?.location?.lesson ?? null : null;
	const lastIndex = lastStudied?.location ? lessons.findIndex((node) => node.path === lastStudied.location!.lesson.path) : -1;
	const currentPath = hasStarted ? (resumeLesson ?? lessons.slice(lastIndex + 1).find((node) => !isDone(node)))?.path ?? null : null;
	const continuePath = currentPath ?? lastStudied?.location?.lesson.path ?? entry?.next?.path;
	const lessonStates = lessons.map((node): SegmentState => (isDone(node) ? 'done' : node.path === currentPath ? 'current' : 'pending'));
	const doneLessons = lessonStates.filter((state) => state === 'done').length;
	const courseNotes = (notes.data ?? []).filter((note) => findLessonLocation(catalog.data ?? null, note.mediaId)?.courseId === courseId);
	const customCategory = customCategories[course.courseId || course.path]?.trim() || null;
	const shownCategory = customCategory ?? (category && category.toLowerCase() !== 'library' ? category : null);
	const primaryLabel = resumeLesson && resumeAt
		? tr(`Resume ${resumeLesson.title} at ${formatDuration(resumeAt)}`, `Retomar ${resumeLesson.title} em ${formatDuration(resumeAt)}`)
		: lastStudied ? tr('Continue studying', 'Continuar estudando') : tr('Start course', 'Começar curso');

	return (
		<div className="space-y-8">
			<section
				data-course-hero
				className="relative grid grid-cols-[minmax(0,1fr)_5.5rem] items-start gap-x-4 gap-y-5 border-b border-[var(--study-rule)] pb-7 sm:grid-cols-[minmax(0,1fr)_12rem] lg:grid-cols-[minmax(0,1fr)_272px] lg:gap-x-10 lg:pt-2"
			>
				<div className="col-start-2 row-start-1 pt-2 lg:row-span-3 lg:pt-0">
					<CourseCover course={course} profile={user?.userId ?? ''} eager map done={doneLessons} total={lessons.length} className="aspect-[16/10] w-full rounded-[var(--radius-lg)]" />
				</div>
				<div className="col-start-1 row-start-1 min-w-0 space-y-2.5">
					<p className="text-[13px] text-muted-foreground">
						<Link to={libraryReturn()} className="inline-flex min-h-11 items-center hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('My learning', 'Meu aprendizado')}</Link> / {course.title}
					</p>
					<h1 className="font-heading text-2xl leading-tight font-semibold tracking-tight text-balance sm:text-[2.5rem] sm:leading-[1.08]">{course.title}</h1>
					<div role="group" aria-label={tr('Course at a glance', 'Resumo do curso')} className="flex flex-wrap items-center gap-x-3.5 gap-y-1.5 text-sm text-muted-foreground">
						{counts.modules > 0 && <span><b className="font-semibold text-foreground tabular-nums">{counts.modules}</b> {counts.modules === 1 ? tr('module', 'módulo') : tr('modules', 'módulos')}</span>}
						{counts.lessons > 0 && <span><b className="font-semibold text-foreground tabular-nums">{counts.lessons}</b> {counts.lessons === 1 ? tr('lesson', 'aula') : tr('lessons', 'aulas')}</span>}
						{materials.length > 0 && <span><b className="font-semibold text-foreground tabular-nums">{materials.length}</b> {materials.length === 1 ? tr('material', 'material') : tr('materials', 'materiais')}</span>}
						{lessons.length > 0 && <span><b className="font-semibold text-foreground tabular-nums">{doneLessons}</b> {tr(`of ${lessons.length} done`, `de ${lessons.length} concluídas`)}</span>}
						<button type="button" onClick={() => setCategorizing(true)} className="inline-flex min-h-11 items-center gap-1.5 rounded-full border border-[var(--study-rule)] bg-card px-3 py-0.5 text-xs font-medium text-muted-foreground transition-colors hover:border-[var(--border-strong)] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none sm:min-h-9 sm:px-2.5">
							<Tag className="size-3 shrink-0" aria-hidden="true" />
							{shownCategory ?? tr('Add category', 'Adicionar categoria')}
						</button>
						<CourseCategoryDialog course={course} courseId={courseId} open={categorizing} onOpenChange={setCategorizing} />
					</div>
				</div>
				<div className="col-span-2 row-start-2 grid grid-cols-2 gap-2 lg:col-span-1 lg:col-start-1 lg:flex lg:flex-wrap lg:items-center">
					{continuePath ? (
						<Link to={path(href('course', courseId, continuePath, resumeLesson ? resumeAt : null))} className="col-span-2 inline-flex min-h-11 items-center justify-center rounded-[var(--radius)] bg-primary px-5 text-center text-base font-semibold whitespace-nowrap text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none lg:col-span-auto lg:flex-none">
							{primaryLabel}
						</Link>
					) : documents.length > 0 ? (
						<Link to={path(href('course', courseId, documents[0].path))} className="col-span-2 inline-flex min-h-11 items-center justify-center rounded-[var(--radius)] bg-primary px-5 text-base font-semibold whitespace-nowrap text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none lg:col-span-auto lg:flex-none">
							{tr('Start reading', 'Começar a ler')}
						</Link>
					) : (
						<p className="text-sm text-muted-foreground">{tr('No lessons available', 'Nenhuma aula disponível')}</p>
					)}
					{studySyncSupported && entry && (
						<Button variant="outline" className="min-w-0 whitespace-nowrap" aria-pressed={entry.favorite} disabled={toggleFavorite.isPending} onClick={() => toggleFavorite.mutate({ courseId, favorite: !entry.favorite })}>
							<Star className={cn(entry.favorite && 'fill-current')} />
							{entry.favorite ? tr('Favorited', 'Favoritado') : tr('Favorite course', 'Favoritar curso')}
						</Button>
					)}
					{user?.isAdmin && !course.remote && <CourseManagement course={course} courseId={courseId} lessons={counts.lessons} className="min-w-0 whitespace-nowrap" onRemoved={() => navigate('/', { replace: true })} />}
					{toggleFavorite.isError && <p role="alert" className="col-span-2 w-full text-sm text-destructive">{tr('Could not update favorites. Try again.', 'Não foi possível atualizar os favoritos. Tente novamente.')}</p>}
				</div>
				<div className="col-span-2 row-start-3 space-y-3 lg:col-span-1 lg:col-start-1">
					{lessons.length > 0 && <LessonSegments done={doneLessons} total={lessons.length} states={lessonStates} className="h-2.5 max-w-md" />}
					{lessons.length > 0 && doneLessons === lessons.length && <p role="status" className="text-sm font-medium text-primary">{tr('Course complete. Well done.', 'Curso concluído. Muito bem.')}</p>}
					{lastStudied?.location && (
						<p className="text-sm text-muted-foreground">
							{tr('Last lesson:', 'Última aula:')} <span className="text-foreground">{displayLessonTitle(lastStudied.location.lesson.title, lastStudied.location.moduleTitle)}</span>, {lastStudied.item.completed ? tr('completed', 'concluída') : tr(`stopped at ${formatDuration(lastStudied.item.positionSeconds)}`, `parou em ${formatDuration(lastStudied.item.positionSeconds)}`)}
						</p>
					)}
				</div>
			</section>

			<Tabs defaultValue="content">
				<TabsList variant="line" aria-label={tr('Course sections', 'Seções do curso')}>
					<TabsTrigger value="content">{tr('Content', 'Conteúdo')}</TabsTrigger>
					<TabsTrigger value="materials">{tr('Materials', 'Materiais')}{materials.length > 0 && <span className="tabular-nums">{materials.length}</span>}</TabsTrigger>
					{studySyncSupported && <TabsTrigger value="notes">{tr('Notes', 'Notas')}</TabsTrigger>}
				</TabsList>
				<TabsContent value="content" className="grid gap-10 pt-5 lg:grid-cols-[minmax(0,1fr)_300px]">
					<div className="min-w-0 space-y-4">
						{!readingOnly && concerns.length > 0 && <StructureNotice concerns={concerns} />}
						{course.children.length > 0 && !readingOnly ? (
							<div className="space-y-3">
								<div className="flex flex-wrap items-center justify-between gap-2">
									<div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
										<div role="search" className="relative min-w-0 flex-1 sm:max-w-xs">
											<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
											<Input type="search" value={lessonQuery} onChange={(event) => { setLessonQuery(event.target.value); if (event.target.value) setReorderMode(false); }} placeholder={tr('Find a lesson', 'Buscar aula')} aria-label={tr('Find a lesson', 'Buscar aula')} className="pl-9" />
										</div>
										{studySyncSupported && lessons.length > 0 && (
											<Button type="button" variant={incompleteOnly ? 'secondary' : 'outline'} size="sm" aria-pressed={incompleteOnly} onClick={() => { const next = !incompleteOnly; setIncompleteOnly(next); if (next) setReorderMode(false); }}>
												{tr('Incomplete only', 'Apenas pendentes')}
											</Button>
										)}
									</div>
									<span className="flex flex-wrap items-center gap-1">
										{canReorder && (
											<Button type="button" variant={reorderMode ? 'secondary' : 'ghost'} size="sm" aria-pressed={reorderMode} onClick={() => { const next = !reorderMode; if (next) { setLessonQuery(''); setIncompleteOnly(false); } setReorderMode(next); }}>
												<ArrowUpDown className="size-3.5" aria-hidden="true" />{reorderMode ? tr('Done reordering', 'Concluir ordenação') : tr('Reorder', 'Reordenar')}
											</Button>
										)}
										<Button variant="ghost" size="sm" onClick={() => setSignal(({ n }) => ({ open: true, n: n + 1 }))}>{tr('Expand all', 'Expandir tudo')}</Button>
										<Button variant="ghost" size="sm" onClick={() => setSignal(({ n }) => ({ open: false, n: n + 1 }))}>{tr('Collapse all', 'Recolher tudo')}</Button>
									</span>
								</div>
								{setCourseOrder.error && <p role="alert" className="text-sm text-destructive">{setCourseOrder.error.message}</p>}
								{batchComplete.error && <p role="alert" className="text-sm text-destructive">{batchComplete.error.message}</p>}
								{filteredNodes.length === 0 ? (
									<p role="status" className="py-4 text-sm text-muted-foreground">{tr('No lessons match that filter.', 'Nenhuma aula corresponde a esse filtro.')}</p>
								) : (
									<Curriculum
										nodes={filteredNodes}
										courseId={courseId}
										completed={completed}
										favorites={favoriteMedia}
										activePath={currentPath}
										resumeSeconds={resumeAt}
										signal={needle ? { open: true, n: signal.n + needle.length + 1 } : signal}
										reorderMode={reorderMode && !filtering}
										reordering={setCourseOrder.isPending}
										onMove={(parent, children) => setCourseOrder.mutate({ courseId, parent, children })}
										onBatchComplete={studySyncSupported ? (mediaIds, value) => batchComplete.mutate({ mediaIds, completed: value }) : undefined}
										batchPending={batchComplete.isPending}
									/>
								)}
							</div>
						) : documents.length === 0 ? <p className="text-muted-foreground">{tr('No lessons found in this course yet.', 'Ainda não encontramos aulas neste curso.')}</p> : null}
						{documents.length > 0 && (
							<section aria-labelledby="course-documents" className="space-y-4">
								<h2 id="course-documents" className="flex items-baseline gap-2 text-lg font-bold">
									{readingOnly ? tr('Reading list', 'Lista de leitura') : tr('Documents', 'Documentos')}
									<span className="text-sm font-normal text-muted-foreground tabular-nums">{documents.length}</span>
								</h2>
								{documentGroups.map(([folder, files]) => (
									<div key={folder || 'root'} className="space-y-2">
										{folder && <h3 className="text-xs font-semibold tracking-wide text-muted-foreground">{folder.split('/').pop()!.replace(/_/g, ' ')}</h3>}
										<ul className="divide-y rounded-lg border bg-card">
											{files.map((file) => (
												<li key={file.path}>
													<Link to={path(href('course', courseId, file.path))} className="flex min-h-11 items-center gap-3 px-4 py-2 text-sm font-medium text-primary hover:underline focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
														<FileText className="size-4 shrink-0" aria-hidden="true" />
														<span className="min-w-0 break-words">{file.name}</span>
													</Link>
												</li>
											))}
										</ul>
									</div>
								))}
							</section>
						)}
					</div>
					<CourseAside course={course} materials={materials} notes={courseNotes} doneCount={doneMediaIds.length} resetting={batchComplete.isPending} onResetProgress={studySyncSupported && doneMediaIds.length > 0 ? () => setConfirmReset(true) : undefined} />
				</TabsContent>
				<TabsContent value="materials" className="pt-4">
					<Materials files={materials} courseId={courseId} />
				</TabsContent>
				{studySyncSupported && (
					<TabsContent value="notes" className="pt-4">
						<CourseNotes course={course} courseId={courseId} />
					</TabsContent>
				)}
			</Tabs>
			<AlertDialog open={confirmReset} onOpenChange={setConfirmReset}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{tr(`Reset progress in “${course.title}”?`, `Reiniciar o progresso em “${course.title}”?`)}</AlertDialogTitle>
						<AlertDialogDescription>
							{tr(
								`${doneMediaIds.length} ${doneMediaIds.length === 1 ? 'completed lesson' : 'completed lessons'} will be unmarked as done. Your notes and flashcards are kept.`,
								`${doneMediaIds.length} ${doneMediaIds.length === 1 ? 'aula concluída será desmarcada' : 'aulas concluídas serão desmarcadas'}. Suas notas e cartões serão mantidos.`
							)}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>{tr('Cancel', 'Cancelar')}</AlertDialogCancel>
						<AlertDialogAction disabled={batchComplete.isPending} onClick={() => batchComplete.mutate({ mediaIds: doneMediaIds, completed: false }, { onSettled: () => setConfirmReset(false) })}>
							{tr('Reset progress', 'Reiniciar progresso')}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</div>
	);
}

/** The course facts the page already holds: your notes, its materials, and where it lives on disk. */
function CourseAside({ course, materials, notes, doneCount, resetting, onResetProgress }: { course: CatalogNode; materials: FileEntry[]; notes: { mediaId: string }[]; doneCount: number; resetting: boolean; onResetProgress?: () => void }) {
	const shown = materials.slice(0, 4);
	const noteLessons = new Set(notes.map((note) => note.mediaId)).size;
	const extension = (name: string) => (name.includes('.') ? name.slice(name.lastIndexOf('.') + 1).toUpperCase() : '');
	return (
		<aside aria-label={tr('About this course', 'Sobre este curso')} className="space-y-5 text-sm lg:pt-1">
			{notes.length > 0 && (
				<section aria-labelledby="course-aside-notes" className="space-y-1 border-t border-[var(--study-rule)] pt-5 first:border-t-0 first:pt-0">
					<h2 id="course-aside-notes" className="text-[12.5px] font-medium text-muted-foreground">{tr('Your notes', 'Suas notas')}</h2>
					<p className="text-foreground/85">{tr(`${notes.length} ${notes.length === 1 ? 'note' : 'notes'} in ${noteLessons} ${noteLessons === 1 ? 'lesson' : 'lessons'}`, `${notes.length} ${notes.length === 1 ? 'nota' : 'notas'} em ${noteLessons} ${noteLessons === 1 ? 'aula' : 'aulas'}`)}</p>
				</section>
			)}
			{materials.length > 0 && (
				<section aria-labelledby="course-aside-materials" className="space-y-2 border-t border-[var(--study-rule)] pt-5 first:border-t-0 first:pt-0">
					<h2 id="course-aside-materials" className="text-[12.5px] font-medium text-muted-foreground">{tr('Materials', 'Materiais')}</h2>
					<ul className="space-y-1.5">
						{shown.map((file) => (
							<li key={file.path} className="flex items-center gap-2.5 text-foreground/85">
								{extension(file.name) && <span className="shrink-0 rounded border border-border px-1 text-[10.5px] font-semibold text-muted-foreground">{extension(file.name)}</span>}
								<span className="min-w-0 break-words">{file.name}</span>
							</li>
						))}
					</ul>
					{materials.length > shown.length && <p className="text-muted-foreground">{tr(`${materials.length - shown.length} more in Materials`, `Mais ${materials.length - shown.length} em Materiais`)}</p>}
				</section>
			)}
			{course.sourceFolder && (
				<section aria-labelledby="course-aside-folder" className="space-y-1 border-t border-[var(--study-rule)] pt-5 first:border-t-0 first:pt-0">
					<h2 id="course-aside-folder" className="text-[12.5px] font-medium text-muted-foreground">{tr('Source folder', 'Pasta de origem')}</h2>
					<p className="break-all text-foreground/85">{course.sourceFolder === 'library' ? tr('Main library', 'Biblioteca principal') : course.sourceFolder}</p>
					<p className="text-muted-foreground">{tr('Read only. Your files are never changed.', 'Somente leitura. Seus arquivos nunca são alterados.')}</p>
				</section>
			)}
			{onResetProgress && doneCount > 0 && (
				<section aria-labelledby="course-aside-reset" className="space-y-2 border-t border-[var(--study-rule)] pt-5 first:border-t-0 first:pt-0">
					<h2 id="course-aside-reset" className="text-[12.5px] font-medium text-muted-foreground">{tr('Study progress', 'Progresso de estudo')}</h2>
					<Button type="button" variant="outline" size="sm" disabled={resetting} onClick={onResetProgress}>
						<RotateCcw className="size-3.5" aria-hidden="true" />{tr('Reset course progress', 'Reiniciar progresso do curso')}
					</Button>
				</section>
			)}
		</aside>
	);
}

function CourseNotes({ course, courseId }: { course: CatalogNode; courseId: string }) {
	const client = useQueryClient();
	const notes = useNotes();
	const catalog = useCatalog();
	const [query, setQuery] = useState('');
	const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
	const update = useMutation({
		mutationFn: ({ id, content }: { id: string; content: string }) => transport.updateNote(id, content),
		onSuccess: () => {
			setEditing(null);
			return Promise.all([client.invalidateQueries({ queryKey: ['notes'] })]);
		}
	});
	const items = useMemo(
		() => (notes.data ?? [])
			.map((note) => ({ note, location: findLessonLocation(catalog.data ?? null, note.mediaId) }))
			.filter(({ location }) => location?.courseId === courseId),
		[notes.data, catalog.data, courseId]
	);
	const needle = normalizeForSearch(query);
	const visible = useMemo(
		() => needle
			? items.filter(({ note, location }) => normalizeForSearch(`${location?.lesson.title ?? ''} ${plainTextMarkdown(note.content)}`).includes(needle))
			: items,
		[items, needle]
	);
	const exportMarkdown = useMemo(() => {
		if (!items.length || typeof URL.createObjectURL !== 'function') return null;
		const body = [`# ${course.title}`, '', ...items.map(({ note, location }) => `## ${location!.lesson.title} (${formatDuration(note.positionSeconds)})\n\n${note.content}`)].join('\n\n');
		return URL.createObjectURL(new Blob([body], { type: 'text/markdown;charset=utf-8' }));
	}, [course.title, items]);
	useEffect(() => () => { if (exportMarkdown) URL.revokeObjectURL(exportMarkdown); }, [exportMarkdown]);
	if (notes.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>;
	if (!items.length) return <p className="text-muted-foreground">{tr(`No notes in ${course.title} yet. Notes you take in a lesson appear here.`, `Ainda não há notas em ${course.title}. As notas que você fizer numa aula aparecem aqui.`)}</p>;
	return (
		<div className="space-y-4">
			<div className="flex flex-wrap items-center justify-between gap-3">
				<div role="search" className="relative min-w-0 flex-1 sm:max-w-xs">
					<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
					<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Filter notes', 'Filtrar notas')} aria-label={tr('Filter notes', 'Filtrar notas')} className="pl-9" />
				</div>
				{exportMarkdown && (
					<a href={exportMarkdown} download={`${course.title.toLowerCase().replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '') || 'course'}-notes.md`} className={buttonVariants({ variant: 'outline', size: 'sm' })}>
						<Download className="size-4" aria-hidden="true" />{tr('Export Markdown', 'Exportar Markdown')}
					</a>
				)}
			</div>
			{update.error && <p role="alert" className="text-sm text-destructive">{update.error.message}</p>}
			{visible.length === 0 ? (
				<p className="text-sm text-muted-foreground">{tr('No notes match that search.', 'Nenhuma nota corresponde à busca.')}</p>
			) : (
				<ul className="space-y-3">
					{visible.map(({ note, location }) => (
						<li key={note.noteId} className="space-y-2 rounded-lg border bg-card p-4">
							<div className="flex flex-wrap items-center justify-between gap-2">
								<Link to={path(href('course', courseId, location!.lesson.path, note.positionSeconds))} className="inline-flex min-h-11 items-center text-sm font-medium text-primary hover:underline">
									{location!.lesson.title} · {formatDuration(note.positionSeconds)}
								</Link>
								{editing?.id !== note.noteId && (
									<Button variant="ghost" size="sm" onClick={() => setEditing({ id: note.noteId, content: note.content })} aria-label={tr(`Edit note in ${location!.lesson.title}`, `Editar nota em ${location!.lesson.title}`)}>
										<Pencil aria-hidden="true" />{tr('Edit', 'Editar')}
									</Button>
								)}
							</div>
							{editing?.id === note.noteId ? (
								<form className="space-y-2" onSubmit={(event) => { event.preventDefault(); if (editing.content.trim() && !update.isPending) update.mutate({ id: editing.id, content: editing.content.trim() }); }}>
									<NoteEditor id={`course-note-${note.noteId}`} label={tr('Note text', 'Texto da nota')} focusOnOpen rows={4} value={editing.content} onChange={(content) => setEditing({ ...editing, content })} />
									<div className="flex justify-end gap-2">
										<Button type="button" variant="outline" size="sm" disabled={update.isPending} onClick={() => setEditing(null)}>{tr('Cancel', 'Cancelar')}</Button>
										<Button type="submit" size="sm" disabled={update.isPending || !editing.content.trim()}>{update.isPending ? tr('Saving…', 'Salvando…') : tr('Save', 'Salvar')}</Button>
									</div>
								</form>
							) : (
								<Markdown text={note.content} className="text-sm leading-relaxed" />
							)}
						</li>
					))}
				</ul>
			)}
		</div>
	);
}

/** The library as the learner left it (filter and search), or its start. */
function libraryReturn(): string {
	try {
		const saved = safeStorage()?.getItem('learnvault-library-return');
		return saved ? `/?${saved}` : '/';
	} catch {
		return '/';
	}
}

/** Why the course's organization may not match what the learner expects, in plain words. */
function StructureNotice({ concerns }: { concerns: string[] }) {
	const headline = tr('Check your course files', 'Confira os arquivos do curso');
	return (
		<div role="note" className="max-w-xl space-y-1 text-sm">
			<p className="font-medium">{headline}</p>
			<ul className="list-disc space-y-0.5 pl-5 text-muted-foreground">
				{concerns.map((concern) => <li key={concern}>{concernText(concern)}</li>)}
			</ul>
		</div>
	);
}

function filterCurriculum(nodes: CatalogNode[], needle: string, incompleteOnly: boolean, completed: ReadonlySet<string>): CatalogNode[] {
	return nodes.flatMap((node) => {
		if (node.role === 'lesson') {
			const matchesText = !needle || normalizeForSearch(node.title).includes(needle);
			const mediaId = lessonMediaId(node);
			const matchesPending = !incompleteOnly || !mediaId || !completed.has(mediaId);
			return matchesText && matchesPending ? [node] : [];
		}
		const children = filterCurriculum(node.children, needle, incompleteOnly, completed);
		return children.length > 0 ? [{ ...node, children }] : [];
	});
}

function concernText(concern: string): string {
	switch (concern) {
		case 'no_lessons': return tr('No lesson has a video, so the lesson list may be empty.', 'Nenhuma aula tem vídeo, então a lista de aulas pode estar vazia.');
		case 'loose_media': return tr('Some videos are outside any lesson, so they are not listed as lessons.', 'Alguns vídeos estão fora de qualquer aula, então não aparecem como aulas.');
		default: return tr('Some files aren’t listed as lessons or materials.', 'Alguns arquivos não aparecem como aulas nem como materiais.');
	}
}
