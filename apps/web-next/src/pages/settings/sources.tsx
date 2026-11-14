import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { PasswordField } from '@/components/password-field';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import { sourceFailure } from '@/lib/source-failure';
import { SourceCourses } from '@/pages/settings/source-courses';
import { transport, type RemoteSource, type SourceConnectionTest } from '@/lib/transport';

const sourcesKey = ['admin-sources'] as const;
const SCHEMES = ['dav', 'davs', 'smb', 'http', 'https'];
const PLAIN_SCHEMES = ['dav', 'http'];
const MAX_NAME = 120;
const EMPTY_FORM = { name: '', url: '', username: '', password: '' };

/** Mirrors the server's URL rules so a typo is caught before the request (crates/server/src/webdav.rs). */
function urlIssue(value: string): string | null {
	const url = value.trim();
	if (!url.includes('://') || !SCHEMES.includes(url.split('://')[0].toLowerCase())) {
		return tr('Use a dav://, davs://, smb://, http:// or https:// address.', 'Use um endereço dav://, davs://, smb://, http:// ou https://.');
	}
	try {
		const parsed = new URL(url);
		if (!parsed.hostname) return tr('Enter the server host in the address.', 'Informe o host do servidor no endereço.');
		if (parsed.username || parsed.password) return tr('Remove the username and password from the address; use the fields below.', 'Remova o usuário e a senha do endereço; use os campos abaixo.');
		if (parsed.search || parsed.hash) return tr('Remove the query string and fragment from the address.', 'Remova a consulta e o fragmento do endereço.');
		if (parsed.protocol === 'smb:' && !parsed.pathname.replace(/\//g, '')) return tr('Add the share name after the host, as in smb://server/share/folder.', 'Informe o nome do compartilhamento após o host, como em smb://servidor/compartilhamento/pasta.');
	} catch {
		return tr('Enter a valid remote address.', 'Informe um endereço remoto válido.');
	}
	return null;
}

/** Plain http/dav carry the Basic credentials in clear text; the server allows it, so warn here. */
function isPlainScheme(value: string): boolean {
	return PLAIN_SCHEMES.includes(value.split('://')[0]?.toLowerCase() ?? '');
}

function testMessage(result: SourceConnectionTest): string {
	const count = result.entries ?? 0;
	return tr(`Reachable · ${count} ${count === 1 ? 'entry' : 'entries'}`, `Acessível · ${count} ${count === 1 ? 'entrada' : 'entradas'}`);
}

/**
 * Remote WebDAV sources (server only). The list never carries the secret: it says whether one
 * is stored, and a broken key is reported instead of silently testing without a password.
 */
export function SourceSettings() {
	const client = useQueryClient();
	const sources = useQuery({ queryKey: sourcesKey, queryFn: () => transport.listSources?.() ?? Promise.resolve([]) });
	const [form, setForm] = useState(EMPTY_FORM);
	const [message, setMessage] = useState('');
	const create = useMutation({
		mutationFn: () => transport.createSource!({
			name: form.name.trim(),
			url: form.url.trim(),
			username: form.username.trim() || undefined,
			password: form.password || undefined
		}),
		onSuccess: () => { setForm(EMPTY_FORM); setMessage(tr('Source added.', 'Fonte adicionada.')); return client.invalidateQueries({ queryKey: sourcesKey }); }
	});

	const nameIssue = form.name && [...form.name.trim()].length > MAX_NAME ? tr(`Use at most ${MAX_NAME} characters.`, `Use no máximo ${MAX_NAME} caracteres.`) : null;
	const addressIssue = form.url ? urlIssue(form.url) : null;
	const credentialsIssue = form.password && !form.username.trim() ? tr('A username is required when a password is set.', 'Um usuário é obrigatório quando há senha.') : null;
	const blocked = Boolean(nameIssue || addressIssue || credentialsIssue);

	const submit = (event: FormEvent) => {
		event.preventDefault();
		if (!blocked && !create.isPending) create.mutate();
	};
	const error = sources.error ?? create.error;

	return (
		<div className="space-y-6">
			<p className="text-sm text-muted-foreground">{tr('A remote source is a read-only WebDAV or SMB folder. Its password is stored encrypted and is never shown again.', 'Uma fonte remota é uma pasta WebDAV ou SMB somente leitura. A senha é guardada cifrada e nunca é mostrada de novo.')}</p>
			{error && <p role="alert" className="text-sm text-destructive">{error.message}</p>}
			<p role="status" aria-live="polite" className="text-sm empty:hidden">{message}</p>

			{sources.isPending ? (
				<p role="status" className="text-sm text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>
			) : (sources.data?.length ?? 0) === 0 ? (
				<p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">{tr('No remote sources yet.', 'Nenhuma fonte remota ainda.')}</p>
			) : (
				<ul className="divide-y rounded-lg border bg-card">
					{sources.data!.map((source) => <SourceRow key={source.id} source={source} onMessage={setMessage} onRemoved={() => { setMessage(tr(`Source “${source.name}” removed.`, `Fonte “${source.name}” removida.`)); void client.invalidateQueries({ queryKey: sourcesKey }); }} />)}
				</ul>
			)}

			<form onSubmit={submit} className="space-y-4 rounded-lg border bg-card p-4">
				<h3 className="font-medium">{tr('Add a source', 'Adicionar uma fonte')}</h3>
				<div className="grid gap-4 sm:grid-cols-2">
					<div className="space-y-2">
						<Label htmlFor="source-name">{tr('Name', 'Nome')}</Label>
						<Input id="source-name" required maxLength={MAX_NAME} autoComplete="off" value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} aria-invalid={Boolean(nameIssue) || undefined} />
						{nameIssue && <p className="text-sm text-destructive">{nameIssue}</p>}
					</div>
					<div className="space-y-2">
						<Label htmlFor="source-url">{tr('Address', 'Endereço')}</Label>
						<Input id="source-url" required type="text" inputMode="url" spellCheck={false} placeholder={tr('davs://server/dav/', 'davs://servidor/dav/')} value={form.url} onChange={(event) => setForm({ ...form, url: event.target.value })} aria-invalid={Boolean(addressIssue) || undefined} aria-describedby={addressIssue ? 'source-url-error' : 'source-url-hint'} />
						{addressIssue && <p id="source-url-error" className="text-sm text-destructive">{addressIssue}</p>}
						{!addressIssue && form.url.toLowerCase().startsWith('smb://') && <p id="source-url-hint" className="text-sm text-muted-foreground">{tr('Format: smb://server/share/folder. The share is required; the connection is read-only.', 'Formato: smb://servidor/compartilhamento/pasta. O compartilhamento é obrigatório; a conexão é somente leitura.')}</p>}
						{!addressIssue && isPlainScheme(form.url) && <p id="source-url-hint" className="text-sm text-muted-foreground">{tr('Plain http:// and dav:// send the password without transport encryption.', 'http:// e dav:// enviam a senha sem criptografia de transporte.')}</p>}
					</div>
					<div className="space-y-2">
						<Label htmlFor="source-username">{tr('Username (optional)', 'Usuário (opcional)')}</Label>
						<Input id="source-username" autoComplete="username" autoCapitalize="none" spellCheck={false} value={form.username} onChange={(event) => setForm({ ...form, username: event.target.value })} />
					</div>
					<PasswordField id="source-password" label={tr('Password (optional)', 'Senha (opcional)')} autoComplete="new-password" value={form.password} onChange={(event) => setForm({ ...form, password: event.target.value })} />
				</div>
				{credentialsIssue && <p className="text-sm text-destructive">{credentialsIssue}</p>}
				<Button type="submit" disabled={create.isPending || blocked}>{create.isPending ? tr('Adding…', 'Adicionando…') : tr('Add source', 'Adicionar fonte')}</Button>
			</form>
		</div>
	);
}

function SourceRow({ source, onRemoved, onMessage }: { source: RemoteSource; onRemoved: () => void; onMessage: (message: string) => void }) {
	const [confirming, setConfirming] = useState(false);
	const test = useMutation({ mutationFn: () => transport.testSource!(source.id) });
	const remove = useMutation({
		mutationFn: () => transport.deleteSource!(source.id),
		onSuccess: (removed) => { if (removed) { setConfirming(false); onRemoved(); } }
	});
	return (
		<li className="space-y-3 p-4">
			<div className="flex flex-wrap items-start gap-2">
				<div className="mr-auto min-w-0 space-y-1">
					<p className="font-medium break-words">{source.name}</p>
					<p className="text-sm break-all text-muted-foreground"><span className="font-mono">{source.url}</span>{source.username && ` · ${source.username}`}</p>
				</div>
				<Button variant="outline" size="sm" disabled={test.isPending} onClick={() => test.mutate()}>{test.isPending ? tr('Testing…', 'Testando…') : tr('Test connection', 'Testar conexão')}</Button>
				{!source.credentialError && <SourceCourses source={source} onMessage={onMessage} />}
				<Button variant="ghost" size="sm" disabled={remove.isPending} onClick={() => setConfirming(true)}>{tr('Remove', 'Remover')}</Button>
			</div>
			<p className="text-sm text-muted-foreground">{source.hasPassword ? tr('Password saved', 'Senha salva') : tr('No password', 'Sem senha')}</p>
			{source.credentialError && <p role="alert" className="text-sm text-destructive">{tr('The stored password cannot be read. Restore the encryption key or add the source again.', 'Não é possível ler a senha guardada. Restaure a chave de criptografia ou adicione a fonte de novo.')}</p>}
			{test.data && <p role="status" className={test.data.ok ? 'text-sm' : 'text-sm text-destructive'}>{test.data.ok ? testMessage(test.data) : sourceFailure(test.data.error)}</p>}
			{test.error && <p role="alert" className="text-sm text-destructive">{sourceFailure(null)}</p>}
			{remove.error && <p role="alert" className="text-sm text-destructive">{tr('The source could not be removed. Try again.', 'Não foi possível remover a fonte. Tente novamente.')}</p>}

			<AlertDialog open={confirming} onOpenChange={(open) => !open && !remove.isPending && setConfirming(false)}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{tr(`Remove the source “${source.name}”?`, `Remover a fonte “${source.name}”?`)}</AlertDialogTitle>
						<AlertDialogDescription>{tr('The stored address and credentials are deleted. The remote server and its files are not touched.', 'O endereço e as credenciais guardados são apagados. O servidor remoto e seus arquivos não são tocados.')}</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>{tr('Cancel', 'Cancelar')}</AlertDialogCancel>
						<AlertDialogAction variant="destructive" disabled={remove.isPending} onClick={() => remove.mutate()}>{remove.isPending ? tr('Removing…', 'Removendo…') : tr('Remove source', 'Remover fonte')}</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</li>
	);
}
