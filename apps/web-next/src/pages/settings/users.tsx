import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { PasswordField } from '@/components/password-field';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import { courseIdOf, transport, type CatalogNode, type LocalUser } from '@/lib/transport';

const usersKey = ['admin-users'] as const;

/** Course access for one account: everything, or exactly the ticked courses. */
function AccessPicker({ id, courses, unrestricted, selected, onChange }: { id: string; courses: CatalogNode[]; unrestricted: boolean; selected: string[]; onChange: (next: { unrestricted: boolean; selected: string[] }) => void }) {
	return (
		<fieldset className="space-y-2">
			<legend className="text-sm font-medium">{tr('Course access', 'Acesso a cursos')}</legend>
			<label className="flex min-h-11 items-center gap-3 text-sm">
				<input type="checkbox" className="size-5 accent-[var(--primary)]" checked={unrestricted} onChange={(event) => onChange({ unrestricted: event.target.checked, selected })} />
				{tr('All courses, including ones added later', 'Todos os cursos, inclusive os adicionados depois')}
			</label>
			{!unrestricted && (
				<ul className="grid gap-1 sm:grid-cols-2">
					{courses.map((course) => {
						const courseId = courseIdOf(course);
						return (
							<li key={courseId}>
								<label htmlFor={`${id}-${courseId}`} className="flex min-h-11 items-center gap-3 text-sm">
									<input id={`${id}-${courseId}`} type="checkbox" className="size-5 accent-[var(--primary)]" checked={selected.includes(courseId)} onChange={(event) => onChange({ unrestricted, selected: event.target.checked ? [...selected, courseId] : selected.filter((item) => item !== courseId) })} />
									{course.title}
								</label>
							</li>
						);
					})}
				</ul>
			)}
		</fieldset>
	);
}

/**
 * Local accounts and per-course access (server only). New accounts start with no course
 * access, so nothing is shared by accident (docs/design/security.md).
 */
export function UserSettings({ courses }: { courses: CatalogNode[] }) {
	const client = useQueryClient();
	const users = useQuery({ queryKey: usersKey, queryFn: () => transport.listUsers?.() ?? Promise.resolve([]) });
	const [message, setMessage] = useState('');
	const [username, setUsername] = useState('');
	const [password, setPassword] = useState('');
	const [access, setAccess] = useState({ unrestricted: false, selected: [] as string[] });
	const [removing, setRemoving] = useState<LocalUser | null>(null);
	const refresh = () => client.invalidateQueries({ queryKey: usersKey });

	const create = useMutation({
		mutationFn: () => transport.createUser!(username.trim(), password, access.unrestricted ? null : access.selected),
		onSuccess: (created) => { setUsername(''); setPassword(''); setAccess({ unrestricted: false, selected: [] }); setMessage(tr(`Account “${created.username}” created.`, `Conta “${created.username}” criada.`)); return refresh(); }
	});
	const remove = useMutation({
		mutationFn: (user: LocalUser) => transport.deleteUser!(user.userId),
		onSuccess: (_, user) => { setRemoving(null); setMessage(tr(`Account “${user.username}” removed.`, `Conta “${user.username}” removida.`)); return refresh(); }
	});
	const submit = (event: FormEvent) => {
		event.preventDefault();
		if (!create.isPending) create.mutate();
	};
	const error = users.error ?? create.error ?? remove.error;

	return (
		<div className="space-y-6">
			<p className="text-sm text-muted-foreground">{tr('Each account has its own progress, notes, flashcards and favourites.', 'Cada conta tem seu próprio progresso, notas, flashcards e favoritos.')}</p>
			{error && <p role="alert" className="text-sm text-destructive">{error.message}</p>}
			<p role="status" aria-live="polite" className="text-sm empty:hidden">{message}</p>
			<ul className="divide-y rounded-lg border bg-card">
				{(users.data ?? []).map((user) => <UserRow key={user.userId} user={user} courses={courses} onSaved={(text) => { setMessage(text); void refresh(); }} onRemove={() => setRemoving(user)} />)}
			</ul>

			<form onSubmit={submit} className="space-y-4 rounded-lg border bg-card p-4">
				<h3 className="font-medium">{tr('New account', 'Nova conta')}</h3>
				<div className="grid gap-4 sm:grid-cols-2">
					<div className="space-y-2">
						<Label htmlFor="new-user-name">{tr('Username', 'Usuário')}</Label>
						<Input id="new-user-name" autoComplete="off" autoCapitalize="none" spellCheck={false} required value={username} onChange={(event) => setUsername(event.target.value)} />
					</div>
					<PasswordField id="new-user-password" label={tr('Password (12+ characters)', 'Senha (12+ caracteres)')} autoComplete="new-password" required minLength={12} value={password} onChange={(event) => setPassword(event.target.value)} />
				</div>
				<AccessPicker id="new-user-access" courses={courses} unrestricted={access.unrestricted} selected={access.selected} onChange={setAccess} />
				<Button type="submit" disabled={create.isPending}>{create.isPending ? tr('Creating…', 'Criando…') : tr('Create account', 'Criar conta')}</Button>
			</form>

			<AlertDialog open={removing !== null} onOpenChange={(open) => !open && setRemoving(null)}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{tr(`Remove the account “${removing?.username}”?`, `Remover a conta “${removing?.username}”?`)}</AlertDialogTitle>
						<AlertDialogDescription>{tr('They are signed out and can no longer sign in. Their study records stay in the database but are no longer reachable from any account. Course files are not touched.', 'A pessoa é desconectada e não consegue mais entrar. Os registros de estudo ficam no banco, mas não podem mais ser acessados por nenhuma conta. Os arquivos dos cursos não são tocados.')}</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>{tr('Cancel', 'Cancelar')}</AlertDialogCancel>
						<AlertDialogAction variant="destructive" disabled={remove.isPending} onClick={() => removing && remove.mutate(removing)}>{tr('Remove account', 'Remover conta')}</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</div>
	);
}

function UserRow({ user, courses, onSaved, onRemove }: { user: LocalUser; courses: CatalogNode[]; onSaved: (message: string) => void; onRemove: () => void }) {
	const [access, setAccess] = useState({ unrestricted: user.allowedCourseIds === null, selected: user.allowedCourseIds ?? [] });
	const save = useMutation({
		mutationFn: () => transport.setUserCourseAccess!(user.userId, access.unrestricted ? null : access.selected),
		onSuccess: () => onSaved(tr(`Access for “${user.username}” saved.`, `Acesso de “${user.username}” salvo.`))
	});
	const summary = user.isAdmin
		? tr('Administrator · all courses', 'Administrador · todos os cursos')
		: user.allowedCourseIds === null ? tr('All courses', 'Todos os cursos') : tr(`${user.allowedCourseIds.length} of ${courses.length} courses`, `${user.allowedCourseIds.length} de ${courses.length} cursos`);
	return (
		<li className="space-y-3 p-4">
			<div className="flex flex-wrap items-center gap-2">
				<p className="mr-auto"><span className="font-medium">{user.username}</span> <span className="text-sm text-muted-foreground">· {summary}</span></p>
				{!user.isAdmin && <Button variant="ghost" size="sm" onClick={onRemove}>{tr('Remove account', 'Remover conta')}</Button>}
			</div>
			{!user.isAdmin && (
				<details>
					<summary className="min-h-11 cursor-pointer content-center text-sm font-medium">{tr(`Change course access for ${user.username}`, `Alterar acesso de ${user.username}`)}</summary>
					<div className="space-y-3 pt-2">
						<AccessPicker id={`access-${user.userId}`} courses={courses} unrestricted={access.unrestricted} selected={access.selected} onChange={setAccess} />
						{save.error && <p role="alert" className="text-sm text-destructive">{save.error.message}</p>}
						<Button size="sm" disabled={save.isPending} onClick={() => save.mutate()}>{tr('Save access', 'Salvar acesso')}</Button>
					</div>
				</details>
			)}
		</li>
	);
}
