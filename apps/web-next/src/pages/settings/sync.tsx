import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { PasswordField } from '@/components/password-field';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import { transport, validSyncServerUrl, type SyncResult } from '@/lib/transport';

const syncKey = ['sync-state'] as const;

function syncFailure(code: string): string {
	switch (code) {
		case 'not_configured': return tr('Set the server address first.', 'Defina o endereço do servidor primeiro.');
		case 'session_expired': return tr('Session expired. Sign in again.', 'Sessão expirada. Entre novamente.');
		case 'credentials_change_required': return tr('Change your password on the server before syncing.', 'Troque sua senha no servidor antes de sincronizar.');
		case 'sync_rejected': return tr('The server rejected some changes; they stay queued.', 'O servidor recusou algumas mudanças; elas continuam na fila.');
		case 'sync_in_progress': return tr('A sync is already running.', 'Já existe uma sincronização em andamento.');
		default: return tr('Sync failed. The queue is kept for the next attempt.', 'A sincronização falhou. A fila é mantida para a próxima tentativa.');
	}
}

function resultText(result: SyncResult): string {
	return tr(
		`Last sync: ${result.pushed} sent, ${result.pulled} received, ${result.pending} pending.`,
		`Última sincronização: ${result.pushed} enviadas, ${result.pulled} recebidas, ${result.pending} pendentes.`
	);
}

/** Server address, sign-in and the local queue. Desktop only: the section is registered there. */
export function SyncSettings() {
	const client = useQueryClient();
	const state = useQuery({ queryKey: syncKey, queryFn: () => transport.syncState!(), retry: false });
	const [draft, setDraft] = useState<string | null>(null);
	const [username, setUsername] = useState('');
	const [password, setPassword] = useState('');
	const data = state.data;

	const url = draft ?? data?.serverUrl ?? '';
	const valid = validSyncServerUrl(url);

	const configure = useMutation({
		mutationFn: (value: string) => transport.syncConfigure!(value.trim()),
		onSuccess: (next) => { setDraft(null); client.setQueryData(syncKey, next); }
	});
	const signIn = useMutation({
		mutationFn: () => transport.syncSignIn!(username.trim(), password),
		onSuccess: (next) => client.setQueryData(syncKey, next),
		// Never keep the password around after the attempt, successful or not.
		onSettled: () => setPassword('')
	});
	const signOut = useMutation({
		mutationFn: () => transport.syncSignOut!(),
		onSuccess: (next) => client.setQueryData(syncKey, next)
	});
	const syncNow = useMutation({
		mutationFn: () => transport.syncNow!(),
		onSuccess: () => state.refetch()
	});

	if (!data) return state.error ? <p role="alert" className="text-sm text-destructive">{state.error.message}</p> : null;

	const error = state.error ?? configure.error ?? signIn.error ?? signOut.error ?? syncNow.error;
	const saveServer = (event: FormEvent) => { event.preventDefault(); if (valid && !configure.isPending) configure.mutate(url); };
	const submitSignIn = (event: FormEvent) => { event.preventDefault(); if (username.trim() && password && !signIn.isPending) signIn.mutate(); };

	return (
		<div className="space-y-6">
			<form onSubmit={saveServer} className="space-y-3">
				<div className="space-y-2">
					<Label htmlFor="sync-server-url">{tr('Server address', 'Endereço do servidor')}</Label>
					<Input id="sync-server-url" type="url" inputMode="url" autoComplete="url" spellCheck={false} placeholder="https://learnvault.example" value={url} aria-invalid={url !== '' && !valid} aria-describedby="sync-server-help" onChange={(event) => setDraft(event.target.value)} />
					<p id="sync-server-help" className="text-sm text-muted-foreground">{tr('The base address of your LearnVault server, with no path, query or login.', 'O endereço base do seu servidor LearnVault, sem caminho, parâmetros ou login.')}</p>
					{url !== '' && !valid && <p role="alert" className="text-sm text-destructive">{tr('Use http:// or https:// followed by the host only.', 'Use http:// ou https:// seguido apenas do host.')}</p>}
				</div>
				<Button type="submit" disabled={!valid || configure.isPending}>{configure.isPending ? tr('Saving…', 'Salvando…') : tr('Save address', 'Salvar endereço')}</Button>
			</form>

			{error && <p role="alert" className="text-sm text-destructive">{error.message}</p>}

			{data.serverUrl !== '' && !data.signedIn && (
				<form onSubmit={submitSignIn} className="space-y-3 rounded-lg border bg-card p-4">
					<h3 className="font-medium">{tr('Sign in to sync', 'Entre para sincronizar')}</h3>
					<div className="grid gap-3 sm:grid-cols-2">
						<div className="space-y-2">
							<Label htmlFor="sync-username">{tr('Username', 'Usuário')}</Label>
							<Input id="sync-username" autoComplete="username" autoCapitalize="none" spellCheck={false} required value={username} onChange={(event) => setUsername(event.target.value)} />
						</div>
						<PasswordField id="sync-password" label={tr('Password', 'Senha')} autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} />
					</div>
					<Button type="submit" disabled={!username.trim() || password === '' || signIn.isPending}>{signIn.isPending ? tr('Signing in…', 'Entrando…') : tr('Sign in', 'Entrar')}</Button>
				</form>
			)}

			{data.signedIn && (
				<div className="space-y-3 rounded-lg border bg-card p-4">
					<p className="text-sm">{tr(`Signed in to ${data.serverUrl}.`, `Conectado a ${data.serverUrl}.`)}</p>
					<p className="text-sm text-muted-foreground">{data.pending === 0 ? tr('Everything is synced.', 'Tudo sincronizado.') : tr(`${data.pending} change(s) waiting to upload.`, `${data.pending} alteração(ões) aguardando envio.`)}</p>
					<p role="status" aria-live="polite" className="text-sm empty:hidden">{syncNow.isPending ? tr('Syncing your changes…', 'Sincronizando suas alterações…') : ''}</p>
					{data.lastResult && !data.lastError && <p className="text-sm text-muted-foreground">{resultText(data.lastResult)}</p>}
					{data.lastError && <p role="alert" className="text-sm text-destructive">{syncFailure(data.lastError)}</p>}
					<div className="flex flex-wrap gap-2">
						<Button disabled={syncNow.isPending} onClick={() => syncNow.mutate()}>{tr('Sync now', 'Sincronizar agora')}</Button>
						<Button variant="outline" disabled={signOut.isPending} onClick={() => signOut.mutate()}>{signOut.isPending ? tr('Signing out…', 'Saindo…') : tr('Sign out', 'Sair')}</Button>
					</div>
					<p className="text-sm text-muted-foreground">{tr('Signing out keeps the queue on this device; it uploads after you sign in again.', 'Sair mantém a fila neste dispositivo; ela é enviada quando você entrar novamente.')}</p>
				</div>
			)}
		</div>
	);
}
