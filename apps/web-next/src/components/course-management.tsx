import { MoreHorizontal } from 'lucide-react';
import { useRef, useState, type ChangeEvent } from 'react';
import { useNavigate } from 'react-router';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { COVER_ACCEPT, MAX_COVER_BYTES, transportKind, type CatalogNode } from '@/lib/transport';
import { useCourseCover, useRemoveCourse, useResetCourseOrder } from '@/app/data';

type Props = {
	course: CatalogNode;
	courseId: string;
	lessons: number;
	compact?: boolean;
	onRemoved?: () => void;
};

function actionError(error: unknown, fallback: string): string {
	if (error instanceof Error && error.message === 'credentials_change_required') {
		return tr('Change your password on the server before managing courses.', 'Troque sua senha no servidor antes de gerenciar cursos.');
	}
	return fallback;
}

/** Admin course actions available from the library and the course header. */
export function CourseManagement({ course, courseId, lessons, compact = false, onRemoved }: Props) {
	const remove = useRemoveCourse();
	const reset = useResetCourseOrder();
	const navigate = useNavigate();
	const [confirming, setConfirming] = useState(false);
	const [message, setMessage] = useState('');
	const cover = useCourseCover();
	const fileInput = useRef<HTMLInputElement>(null);
	const report = { onError: (error: Error) => setMessage(actionError(error, tr('Could not update the course.', 'Não foi possível atualizar o curso.'))) };
	const changeCover = (file?: File) => cover.change.mutate({ courseId, file }, { ...report, onSuccess: () => setMessage(tr('Cover updated.', 'Capa atualizada.')) });
	const pickFile = (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		event.target.value = '';
		if (!file) return;
		if (!COVER_ACCEPT.split(',').includes(file.type)) return setMessage(tr('Choose a PNG, JPEG or WebP image.', 'Escolha uma imagem PNG, JPEG ou WebP.'));
		if (file.size > MAX_COVER_BYTES) return setMessage(tr('The image is larger than 4 MiB.', 'A imagem tem mais de 4 MiB.'));
		changeCover(file);
	};
	const finishRemoval = () => onRemoved ? onRemoved() : navigate('/', { replace: true });

	return (
		<>
			<input ref={fileInput} type="file" accept={COVER_ACCEPT} onChange={pickFile} className="sr-only" tabIndex={-1} aria-hidden="true" />
			<DropdownMenu>
				<DropdownMenuTrigger render={
					<Button
						variant={compact ? 'ghost' : 'outline'}
						size={compact ? 'icon' : 'lg'}
						{...(compact ? { 'aria-label': tr(`Manage ${course.title}`, `Gerenciar ${course.title}`) } : {})}
					/>
				}>
					{compact ? <MoreHorizontal aria-hidden="true" /> : tr('Manage course', 'Gerenciar curso')}
				</DropdownMenuTrigger>
				<DropdownMenuContent align="end">
					<DropdownMenuItem disabled={reset.isPending} onClick={() => reset.mutate(courseId, {
						onSuccess: (changed) => setMessage(changed ? tr('Lesson order restored.', 'Ordem das aulas restaurada.') : tr('Lesson order was already at its default.', 'A ordem das aulas já estava no padrão.')),
						onError: (error) => setMessage(actionError(error, tr('Could not restore lesson order.', 'Não foi possível restaurar a ordem das aulas.')))
					})}>
						{tr('Restore lesson order', 'Restaurar ordem das aulas')}
					</DropdownMenuItem>
					<DropdownMenuItem disabled={cover.change.isPending} onClick={() => (transportKind === 'desktop' ? changeCover() : fileInput.current?.click())}>
						{course.coverVersion ? tr('Change cover', 'Trocar capa') : tr('Add cover', 'Adicionar capa')}
					</DropdownMenuItem>
					{Boolean(course.coverVersion) && (
						<DropdownMenuItem disabled={cover.remove.isPending} onClick={() => cover.remove.mutate(courseId, {
							...report,
							onSuccess: (removed) => setMessage(removed ? tr('Cover removed.', 'Capa removida.') : tr('This course has no custom cover.', 'Este curso não tem uma capa personalizada.'))
						})}>
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
					{remove.error && <p role="alert" className="text-sm text-destructive">{actionError(remove.error, tr('Could not remove the course.', 'Não foi possível remover o curso.'))}</p>}
					<AlertDialogFooter>
						<AlertDialogCancel>{tr('Cancel', 'Cancelar')}</AlertDialogCancel>
						<AlertDialogAction variant="destructive" disabled={remove.isPending} onClick={() => remove.mutate(courseId, { onSuccess: finishRemoval })}>
							{remove.isPending ? tr('Removing…', 'Removendo…') : tr('Remove course', 'Remover curso')}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	);
}
