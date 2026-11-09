import { Star } from 'lucide-react';
import { useMemo, useRef, useState, type ChangeEvent } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { CourseCover } from '@/components/course-cover';
import { Curriculum } from '@/components/curriculum';
import { Materials } from '@/components/materials';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Button, buttonVariants } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useLocale } from '@/hooks/use-locale';
import { collectMaterials, countCatalogNode, formatCount, formatCourseMeta } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findCourse, findLessonLocation, href } from '@/lib/router';
import { COVER_ACCEPT, MAX_COVER_BYTES, transportKind, type CatalogNode } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { LessonPage } from './lesson';
import { useCatalog, useCourseCover, useFavorites, useNotes, useRecent, useRemoveCourse, useResetCourseOrder, useSession, useToggleCourseFavorite } from '@/app/data';

const path = (hash: string) => hash.replace(/^#/, '');

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
	return (
		<div className="space-y-4">
			<h1 className="text-2xl font-semibold">{tr('Course not found', 'Curso não encontrado')}</h1>
			<p className="text-muted-foreground">{tr('It may have been removed from the library or you may not have access to it.', 'Ele pode ter sido removido da biblioteca ou você pode não ter acesso a ele.')}</p>
			<Link to="/" className={buttonVariants({ variant: 'outline' })}>{tr('Back to My learning', 'Voltar para Meu aprendizado')}</Link>
		</div>
	);
}

function CourseOverview({ course, courseId }: { course: CatalogNode; courseId: string }) {
	const session = useSession();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const toggleFavorite = useToggleCourseFavorite();
	const user = session.data?.user;

	const completed = useMemo(() => new Set((recent.data ?? []).filter((item) => item.completed).map((item) => item.mediaId)), [recent.data]);
	const favoriteMedia = useMemo(() => new Set(favorites.data?.mediaIds ?? []), [favorites.data]);
	const entry = useMemo(
		() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set(favorites.data?.courseIds ?? []) }).entries.find((item) => item.courseId === courseId),
		[catalog.data, recent.data, favorites.data, courseId]
	);
	const lastStudied = (recent.data ?? []).map((item) => ({ item, location: findLessonLocation(catalog.data ?? null, item.mediaId) })).find(({ location }) => location?.courseId === courseId);
	const counts = countCatalogNode(course);
	const materials = collectMaterials(course);
	const percent = entry?.total ? Math.round((entry.done / entry.total) * 100) : 0;
	const resumeAt = lastStudied && !lastStudied.item.completed ? lastStudied.item.positionSeconds : null;
	const continuePath = lastStudied?.location && !lastStudied.item.completed ? lastStudied.location.lesson.path : entry?.next?.path;

	return (
		<div className="space-y-8">
			<header className="grid gap-6 md:grid-cols-[minmax(0,20rem)_1fr] md:items-center">
				<CourseCover course={course} profile={user?.userId ?? ''} eager />
				<div className="min-w-0 space-y-4">
					<div className="space-y-1">
						<h1 className="text-3xl font-semibold tracking-tight text-balance">{course.title}</h1>
						<p className="text-muted-foreground">{formatCourseMeta(counts)}</p>
					</div>
					{entry && entry.total > 0 && (
						<div className="max-w-md space-y-1">
							<div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-labelledby="course-progress" className="h-2 overflow-hidden rounded-full bg-muted">
								<div className="h-full bg-primary" style={{ width: `${percent}%` }} />
							</div>
							<p id="course-progress" className="text-sm text-muted-foreground">{tr(`${entry.done} of ${entry.total} lessons completed (${percent}%)`, `${entry.done} de ${entry.total} aulas concluídas (${percent}%)`)}</p>
						</div>
					)}
					{lastStudied?.location && (
						<p className="text-sm">
							{tr('Last lesson:', 'Última aula:')} {lastStudied.location.lesson.title} · {lastStudied.item.completed ? tr('completed', 'concluída') : tr(`stopped at ${formatDuration(lastStudied.item.positionSeconds)}`, `parou em ${formatDuration(lastStudied.item.positionSeconds)}`)}
						</p>
					)}
					<div className="flex flex-wrap gap-2">
						{continuePath ? (
							<Link to={path(href('course', courseId, continuePath, resumeAt))} className={buttonVariants({ size: 'lg' })}>
								{lastStudied ? tr('Continue studying', 'Continuar estudando') : tr('Start course', 'Começar curso')}
							</Link>
						) : (
							<p className="text-sm text-muted-foreground">{tr('No lessons available', 'Nenhuma aula disponível')}</p>
						)}
						{studySyncSupported && entry && (
							<Button variant="outline" size="lg" aria-pressed={entry.favorite} disabled={toggleFavorite.isPending} onClick={() => toggleFavorite.mutate({ courseId, favorite: !entry.favorite })}>
								<Star className={cn(entry.favorite && 'fill-primary text-primary')} />
								{entry.favorite ? tr('Favorited', 'Favoritado') : tr('Favorite course', 'Favoritar curso')}
							</Button>
						)}
						{user?.isAdmin && <ManageCourse course={course} courseId={courseId} lessons={counts.lessons} />}
					</div>
				</div>
			</header>

			<Tabs defaultValue="content">
				<TabsList aria-label={tr('Course sections', 'Seções do curso')}>
					<TabsTrigger value="content">{tr('Content', 'Conteúdo')}</TabsTrigger>
					<TabsTrigger value="materials">{tr('Materials', 'Materiais')}{materials.length > 0 && <span className="tabular-nums">{materials.length}</span>}</TabsTrigger>
					{studySyncSupported && <TabsTrigger value="notes">{tr('Notes', 'Notas')}</TabsTrigger>}
				</TabsList>
				<TabsContent value="content" className="pt-4">
					{course.children.length > 0
						? <Curriculum nodes={course.children} courseId={courseId} completed={completed} favorites={favoriteMedia} activePath={continuePath} />
						: <p className="text-muted-foreground">{tr('This course has not been indexed yet.', 'Este curso ainda não foi indexado.')}</p>}
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

/** Admin actions. Removing only unregisters the course; the dialog states the impact first. */
function ManageCourse({ course, courseId, lessons }: { course: CatalogNode; courseId: string; lessons: number }) {
	const remove = useRemoveCourse();
	const reset = useResetCourseOrder();
	const navigate = useNavigate();
	const [confirming, setConfirming] = useState(false);
	const [message, setMessage] = useState('');
	const cover = useCourseCover();
	const fileInput = useRef<HTMLInputElement>(null);
	const report = { onError: (error: Error) => setMessage(error.message) };
	const changeCover = (file?: File) => cover.change.mutate({ courseId, file }, { ...report, onSuccess: () => setMessage(tr('Cover updated.', 'Capa atualizada.')) });
	const pickFile = (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		event.target.value = '';
		if (!file) return;
		// Checked here for a quick answer; the server validates the bytes again.
		if (!COVER_ACCEPT.split(',').includes(file.type)) return setMessage(tr('Choose a PNG, JPEG or WebP image.', 'Escolha uma imagem PNG, JPEG ou WebP.'));
		if (file.size > MAX_COVER_BYTES) return setMessage(tr('The image is larger than 4 MiB.', 'A imagem tem mais de 4 MiB.'));
		changeCover(file);
	};
	return (
		<>
			<input ref={fileInput} type="file" accept={COVER_ACCEPT} onChange={pickFile} className="sr-only" tabIndex={-1} aria-hidden="true" />
			<DropdownMenu>
				<DropdownMenuTrigger render={<Button variant="outline" size="lg" />}>{tr('Manage course', 'Gerenciar curso')}</DropdownMenuTrigger>
				<DropdownMenuContent align="end">
					<DropdownMenuItem disabled={reset.isPending} onClick={() => reset.mutate(courseId, { onSuccess: () => setMessage(tr('Lesson order restored.', 'Ordem das aulas restaurada.')), onError: (error) => setMessage(error.message) })}>
						{tr('Restore lesson order', 'Restaurar ordem das aulas')}
					</DropdownMenuItem>
					<DropdownMenuItem disabled={cover.change.isPending} onClick={() => (transportKind === 'desktop' ? changeCover() : fileInput.current?.click())}>
						{course.coverVersion ? tr('Change cover', 'Trocar capa') : tr('Add cover', 'Adicionar capa')}
					</DropdownMenuItem>
					{Boolean(course.coverVersion) && (
						<DropdownMenuItem disabled={cover.remove.isPending} onClick={() => cover.remove.mutate(courseId, { ...report, onSuccess: () => setMessage(tr('Cover removed.', 'Capa removida.')) })}>
							{tr('Remove cover', 'Remover capa')}
						</DropdownMenuItem>
					)}
					<DropdownMenuItem variant="destructive" onClick={() => setConfirming(true)}>{tr('Remove course', 'Remover curso')}</DropdownMenuItem>
				</DropdownMenuContent>
			</DropdownMenu>
			<p role="status" aria-live="polite" className="basis-full text-sm text-muted-foreground empty:hidden">{message}</p>
			<AlertDialog open={confirming} onOpenChange={setConfirming}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{tr(`Remove “${course.title}” from the library?`, `Remover “${course.title}” da biblioteca?`)}</AlertDialogTitle>
						<AlertDialogDescription>
							{tr(
								`The course and its ${formatCount(lessons, 'lesson', 'lessons')} leave the library for every user. The original files on disk are not deleted, and importing the folder again brings it back.`,
								`O curso e suas ${formatCount(lessons, 'aula', 'aulas')} saem da biblioteca de todos os usuários. Os arquivos originais no disco não são apagados, e importar a pasta de novo traz o curso de volta.`
							)}
						</AlertDialogDescription>
					</AlertDialogHeader>
					{remove.error && <p role="alert" className="text-sm text-destructive">{remove.error.message}</p>}
					<AlertDialogFooter>
						<AlertDialogCancel>{tr('Cancel', 'Cancelar')}</AlertDialogCancel>
						<AlertDialogAction variant="destructive" disabled={remove.isPending} onClick={() => remove.mutate(courseId, { onSuccess: () => navigate('/', { replace: true }) })}>
							{remove.isPending ? tr('Removing…', 'Removendo…') : tr('Remove course', 'Remover curso')}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	);
}
