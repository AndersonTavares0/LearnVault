import { FileText, Star, Tag } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { safeStorage } from '@/app/preferences';
import { CourseCategoryDialog } from '@/components/course-category-dialog';
import { CourseManagement } from '@/components/course-management';
import { CourseCover } from '@/components/course-cover';
import { Curriculum, lessonMediaId } from '@/components/curriculum';
import { LessonSegments, type SegmentState } from '@/components/lesson-segments';
import { Materials } from '@/components/materials';
import { Button, buttonVariants } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useLocale } from '@/hooks/use-locale';
import { usePageTitle } from '@/hooks/use-page-title';
import { useStore } from '@/hooks/use-store';
import { collectMaterials, countCatalogNode, isLessonMissing } from '@/lib/catalog';
import { courseCategoriesStore, resolveCourseCategory } from '@/lib/course-categories';
import { tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { isPdf } from '@/lib/pdf';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findCourse, findLessonLocation, href } from '@/lib/router';
import type { CatalogNode, FileEntry } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { LessonPage } from './lesson';
import { useCatalog, useFavorites, useNotes, useRecent, useSession, useToggleCourseFavorite } from '@/app/data';

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
	const user = session.data?.user;
	const [signal, setSignal] = useState({ open: false, n: 0 });
	const [categorizing, setCategorizing] = useState(false);
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
	const resumeLesson = resumeAt ? lastStudied?.location?.lesson ?? null : null;
	const lastIndex = lastStudied?.location ? lessons.findIndex((node) => node.path === lastStudied.location!.lesson.path) : -1;
	const currentPath = (resumeLesson ?? lessons.slice(lastIndex + 1).find((node) => !isDone(node)))?.path ?? null;
	const continuePath = currentPath ?? lastStudied?.location?.lesson.path ?? entry?.next?.path;
	const lessonStates = lessons.map((node): SegmentState => (isDone(node) ? 'done' : node.path === currentPath ? 'current' : 'pending'));
	const doneLessons = lessonStates.filter((state) => state === 'done').length;
	const courseNotes = (notes.data ?? []).filter((note) => findLessonLocation(catalog.data ?? null, note.mediaId)?.courseId === courseId);
	const primaryLabel = resumeLesson && resumeAt
		? tr(`Resume ${resumeLesson.title} at ${formatDuration(resumeAt)}`, `Retomar ${resumeLesson.title} em ${formatDuration(resumeAt)}`)
		: lastStudied ? tr('Continue studying', 'Continuar estudando') : tr('Start course', 'Começar curso');

	return (
		<div className="space-y-8">
			<section
				data-course-hero
				className="relative grid grid-cols-[7rem_minmax(0,1fr)] items-start gap-x-4 gap-y-5 border-b border-[var(--study-rule)] pb-8 lg:grid-cols-[minmax(0,1fr)_440px] lg:gap-x-10 lg:bg-[radial-gradient(50%_60%_at_78%_40%,var(--glow),transparent_70%)] lg:pt-2"
			>
				<div className="row-start-1 col-start-1 lg:col-start-2 lg:row-span-3">
					<CourseCover course={course} profile={user?.userId ?? ''} eager done={entry?.done ?? 0} total={entry?.total ?? 0} compact className="aspect-video" />
				</div>
				<div className="col-start-2 row-start-1 min-w-0 space-y-3 lg:col-start-1">
					<p className="text-[13px] text-muted-foreground">
						<Link to={libraryReturn()} className="inline-flex min-h-11 items-center hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('My learning', 'Meu aprendizado')}</Link> / {course.title}
					</p>
					<h1 className="font-heading text-2xl leading-tight font-semibold tracking-tight text-balance sm:text-[2.75rem] sm:leading-[1.05]">{course.title}</h1>
					<div role="group" aria-label={tr('Course at a glance', 'Resumo do curso')} className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
						{counts.modules > 0 && <span><b className="font-semibold text-foreground tabular-nums">{counts.modules}</b> {counts.modules === 1 ? tr('module', 'módulo') : tr('modules', 'módulos')}</span>}
						{counts.lessons > 0 && <span><b className="font-semibold text-foreground tabular-nums">{counts.lessons}</b> {counts.lessons === 1 ? tr('lesson', 'aula') : tr('lessons', 'aulas')}</span>}
						{materials.length > 0 && <span><b className="font-semibold text-foreground tabular-nums">{materials.length}</b> {materials.length === 1 ? tr('material', 'material') : tr('materials', 'materiais')}</span>}
						{lessons.length > 0 && <span><b className="font-semibold text-foreground tabular-nums">{doneLessons}</b> {tr(`of ${lessons.length} done`, `de ${lessons.length} concluídas`)}</span>}
						<button type="button" onClick={() => setCategorizing(true)} className="inline-flex min-h-11 items-center gap-1.5 text-muted-foreground transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none">
							<Tag className="size-3.5 shrink-0" aria-hidden="true" />
							{category ?? tr('Add category', 'Adicionar categoria')}
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
				</div>
				<div className="col-span-2 row-start-3 space-y-3 lg:col-span-1 lg:col-start-1">
					{lessons.length > 0 && <LessonSegments done={doneLessons} total={lessons.length} states={lessonStates} className="h-2.5 max-w-md" />}
					{lessons.length > 0 && doneLessons === lessons.length && <p role="status" className="text-sm font-medium text-primary">{tr('Course complete. Well done.', 'Curso concluído. Muito bem.')}</p>}
					{lastStudied?.location && (
						<p className="text-sm text-muted-foreground">
							{tr('Last lesson:', 'Última aula:')} <span className="text-foreground">{lastStudied.location.lesson.title}</span>, {lastStudied.item.completed ? tr('completed', 'concluída') : tr(`stopped at ${formatDuration(lastStudied.item.positionSeconds)}`, `parou em ${formatDuration(lastStudied.item.positionSeconds)}`)}
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
								<div className="flex flex-wrap items-center justify-end gap-2">
									<span className="flex gap-1">
										<Button variant="ghost" size="sm" onClick={() => setSignal(({ n }) => ({ open: true, n: n + 1 }))}>{tr('Expand all', 'Expandir tudo')}</Button>
										<Button variant="ghost" size="sm" onClick={() => setSignal(({ n }) => ({ open: false, n: n + 1 }))}>{tr('Collapse all', 'Recolher tudo')}</Button>
									</span>
								</div>
								<Curriculum nodes={course.children} courseId={courseId} completed={completed} favorites={favoriteMedia} activePath={currentPath} resumeSeconds={resumeAt} signal={signal} />
							</div>
						) : documents.length === 0 ? <p className="text-muted-foreground">{tr('This course has not been indexed yet.', 'Este curso ainda não foi indexado.')}</p> : null}
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
					<CourseAside course={course} materials={materials} notes={courseNotes} />
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
		</div>
	);
}

/** The course facts the page already holds: your notes, its materials, and where it lives on disk. */
function CourseAside({ course, materials, notes }: { course: CatalogNode; materials: FileEntry[]; notes: { mediaId: string }[] }) {
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
					<h2 id="course-aside-folder" className="text-[12.5px] font-medium text-muted-foreground">{tr('Folder', 'Pasta')}</h2>
					<p className="break-all text-foreground/85">{course.sourceFolder}</p>
					<p className="text-muted-foreground">{tr('Read only. Your files are never changed.', 'Somente leitura. Seus arquivos nunca são alterados.')}</p>
				</section>
			)}
		</aside>
	);
}

function CourseNotes({ course, courseId }: { course: CatalogNode; courseId: string }) {
	const notes = useNotes();
	const catalog = useCatalog();
	const items = (notes.data ?? [])
		.map((note) => ({ note, location: findLessonLocation(catalog.data ?? null, note.mediaId) }))
		.filter(({ location }) => location?.courseId === courseId);
	if (notes.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>;
	if (!items.length) return <p className="text-muted-foreground">{tr(`No notes in ${course.title} yet. Notes you take in a lesson appear here.`, `Ainda não há notas em ${course.title}. As notas que você fizer numa aula aparecem aqui.`)}</p>;
	return (
		<ul className="space-y-3">
			{items.map(({ note, location }) => (
				<li key={note.noteId} className="rounded-lg border bg-card p-4">
					<Link to={path(href('course', courseId, location!.lesson.path, note.positionSeconds))} className="inline-flex min-h-11 items-center text-sm font-medium text-primary hover:underline">
						{location!.lesson.title} · {formatDuration(note.positionSeconds)}
					</Link>
					<p className="whitespace-pre-wrap">{note.content}</p>
				</li>
			))}
		</ul>
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

function concernText(concern: string): string {
	switch (concern) {
		case 'no_lessons': return tr('No lesson has a video, so the lesson list may be empty.', 'Nenhuma aula tem vídeo, então a lista de aulas pode estar vazia.');
		case 'loose_media': return tr('Some videos are outside any lesson, so they are not listed as lessons.', 'Alguns vídeos estão fora de qualquer aula, então não aparecem como aulas.');
		default: return tr('Some files could not be classified.', 'Alguns arquivos não puderam ser classificados.');
	}
}
