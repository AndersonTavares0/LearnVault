import { useMutation, useQueryClient } from '@tanstack/react-query';
import { CircleUser } from 'lucide-react';
import { useEffect, useRef, useState, type ChangeEvent, type FormEvent } from 'react';
import { PasswordField } from '@/components/password-field';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import { transport, type User } from '@/lib/transport';
import { keys } from '@/app/data';
import { safeStorage } from '@/app/preferences';

const AVATAR_TYPES = ['image/png', 'image/jpeg', 'image/webp'];
const MAX_AVATAR_BYTES = 2 * 1024 * 1024;
const MIN_PASSWORD = 12;
const avatarKey = (userId: string) => `learnvault_avatar_${userId}`;

/** The avatar version busts the browser cache after an upload; it is per user and per browser. */
export function readAvatarVersion(userId: string): number {
	try {
		return Number(safeStorage()?.getItem(avatarKey(userId)) ?? 0) || 0;
	} catch {
		return 0;
	}
}

function writeAvatarVersion(userId: string, version: number) {
	try {
		if (version) safeStorage()?.setItem(avatarKey(userId), String(version));
		else safeStorage()?.removeItem(avatarKey(userId));
	} catch {
		// Without storage the avatar still shows until the next reload.
	}
}

/** Profile photo, validated in the browser before upload; the server stays the authority. */
export function AvatarSettings({ user }: { user: User }) {
	const input = useRef<HTMLInputElement>(null);
	const [version, setVersion] = useState(() => readAvatarVersion(user.userId));
	const [preview, setPreview] = useState<{ file: File; url: string } | null>(null);
	const [failed, setFailed] = useState(false);
	const [message, setMessage] = useState('');
	useEffect(() => () => { if (preview) URL.revokeObjectURL(preview.url); }, [preview]);

	const upload = useMutation({
		mutationFn: (file: File) => transport.uploadUserAvatar(file),
		onSuccess: ({ version: next }) => { writeAvatarVersion(user.userId, next); setVersion(next); setPreview(null); setFailed(false); setMessage(tr('Photo updated.', 'Foto atualizada.')); },
		onError: (error) => setMessage(error.message)
	});
	const remove = useMutation({
		mutationFn: () => transport.deleteUserAvatar(),
		onSuccess: () => { writeAvatarVersion(user.userId, 0); setVersion(0); setMessage(tr('Photo removed.', 'Foto removida.')); },
		onError: (error) => setMessage(error.message)
	});

	const choose = (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		event.target.value = '';
		if (!file) return;
		if (!AVATAR_TYPES.includes(file.type.toLowerCase())) return setMessage(tr('Choose a PNG, JPEG or WebP image.', 'Escolha uma imagem PNG, JPEG ou WebP.'));
		if (file.size > MAX_AVATAR_BYTES) return setMessage(tr('The image is larger than 2 MiB. Choose a smaller one.', 'A imagem tem mais de 2 MiB. Escolha uma menor.'));
		setMessage('');
		setPreview({ file, url: URL.createObjectURL(file) });
	};

	// The preview is a `blob:` URL this page made from the chosen file; nothing else is ever shown from it.
	const previewSrc = preview && /^blob:/.test(preview.url) ? preview.url : '';
	const src = previewSrc || (version && !failed ? transport.userAvatarUrl(version) : '');
	return (
		<div className="flex flex-wrap items-center gap-4">
			{src ? <img src={src} alt={tr(`Photo of ${user.username}`, `Foto de ${user.username}`)} onError={() => setFailed(true)} className="size-20 rounded-full object-cover" /> : <CircleUser className="size-20 text-muted-foreground" aria-hidden="true" />}
			<div className="flex flex-wrap gap-2">
				<input ref={input} type="file" accept={AVATAR_TYPES.join(',')} onChange={choose} className="sr-only" id="avatar-file" tabIndex={-1} aria-hidden="true" />
				{preview ? (
					<>
						<Button disabled={upload.isPending} onClick={() => upload.mutate(preview.file)}>{upload.isPending ? tr('Saving…', 'Salvando…') : tr('Save photo', 'Salvar foto')}</Button>
						<Button variant="outline" onClick={() => setPreview(null)}>{tr('Cancel', 'Cancelar')}</Button>
					</>
				) : (
					<>
						<Button variant="outline" onClick={() => input.current?.click()}>{version ? tr('Change photo', 'Trocar foto') : tr('Add photo', 'Adicionar foto')}</Button>
						{version > 0 && <Button variant="ghost" disabled={remove.isPending} onClick={() => remove.mutate()}>{tr('Remove photo', 'Remover foto')}</Button>}
					</>
				)}
			</div>
			<p role="status" aria-live="polite" className="basis-full text-sm text-muted-foreground empty:hidden">{message}</p>
		</div>
	);
}

/** Username and password change; the server issues a new session and ends the others. */
export function CredentialsForm({ user }: { user: User }) {
	const client = useQueryClient();
	const [current, setCurrent] = useState('');
	const [username, setUsername] = useState(user.username);
	const [password, setPassword] = useState('');
	const save = useMutation({
		mutationFn: () => transport.updateCredentials(username.trim(), password, current),
		onSuccess: (updated) => {
			client.setQueryData(keys.session, { user: updated, error: '' });
			setCurrent('');
			setPassword('');
		}
	});
	const tooShort = password.length > 0 && [...password].length < MIN_PASSWORD;
	const submit = (event: FormEvent) => {
		event.preventDefault();
		if (!tooShort && !save.isPending) save.mutate();
	};
	return (
		<form onSubmit={submit} className="max-w-md space-y-4">
			<PasswordField id="current-password" label={tr('Current password', 'Senha atual')} autoComplete="current-password" required value={current} onChange={(event) => setCurrent(event.target.value)} />
			<div className="space-y-2">
				<Label htmlFor="new-username">{tr('Username', 'Usuário')}</Label>
				<Input id="new-username" autoComplete="username" autoCapitalize="none" spellCheck={false} required value={username} onChange={(event) => setUsername(event.target.value)} />
			</div>
			<PasswordField id="new-password" label={tr('New password', 'Nova senha')} autoComplete="new-password" required value={password} onChange={(event) => setPassword(event.target.value)} aria-describedby="new-password-hint" aria-invalid={tooShort || undefined} />
			<p id="new-password-hint" className={tooShort ? 'text-sm text-destructive' : 'text-sm text-muted-foreground'}>{tr(`Use at least ${MIN_PASSWORD} characters. A passphrase is easier to remember.`, `Use ao menos ${MIN_PASSWORD} caracteres. Uma frase-senha é mais fácil de lembrar.`)}</p>
			{save.error && <p role="alert" className="text-sm text-destructive">{save.error.message}</p>}
			{save.isSuccess && <p role="status" className="text-sm">{tr('Credentials updated. Your other sessions were signed out.', 'Credenciais atualizadas. Suas outras sessões foram encerradas.')}</p>}
			<Button type="submit" disabled={save.isPending || tooShort}>{save.isPending ? tr('Saving…', 'Salvando…') : tr('Update credentials', 'Atualizar credenciais')}</Button>
		</form>
	);
}
