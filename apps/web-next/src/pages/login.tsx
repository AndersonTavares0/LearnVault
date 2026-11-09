import { useState, type FormEvent } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router';
import { BrandMark } from '@/components/brand';
import { PasswordField } from '@/components/password-field';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { LOCALE_LABELS, setLocale, tr } from '@/lib/i18n';
import { useSession, useSignIn } from '@/app/data';

/** Returns a same-app path only, never an external URL (open-redirect guard). */
function safeDestination(state: unknown): string {
	const from = (state as { from?: unknown } | null)?.from;
	return typeof from === 'string' && from.startsWith('/') && !from.startsWith('//') && from !== '/login' ? from : '/';
}

export function LoginPage() {
	const locale = useLocale();
	const session = useSession();
	const signIn = useSignIn();
	const navigate = useNavigate();
	const location = useLocation();
	const [username, setUsername] = useState('');
	const [password, setPassword] = useState('');
	const destination = safeDestination(location.state);
	const otherLocale = locale === 'en' ? 'pt-BR' : 'en';

	if (session.data?.user) return <Navigate to={destination} replace />;

	function submit(event: FormEvent<HTMLFormElement>) {
		event.preventDefault();
		if (signIn.isPending) return;
		signIn.mutate({ username, password }, {
			onSuccess: () => navigate(destination, { replace: true }),
			onSettled: () => setPassword('')
		});
	}

	const error = signIn.error?.message || session.data?.error;
	return (
		<main id="main" className="grid min-h-dvh place-items-center px-4 py-10">
			<div className="w-full max-w-sm space-y-6">
				<div className="flex flex-col items-center gap-3 text-center">
					<BrandMark className="size-12 text-primary" />
					<h1 className="text-2xl font-semibold tracking-tight">{tr('Sign in to LearnVault', 'Entrar no LearnVault')}</h1>
				</div>
				<form onSubmit={submit} className="space-y-4 rounded-xl border bg-card p-6" noValidate={false}>
					<div className="space-y-2">
						<Label htmlFor="username">{tr('Username', 'Usuário')}</Label>
						<Input id="username" name="username" autoComplete="username" autoCapitalize="none" spellCheck={false} required value={username} onChange={(event) => setUsername(event.target.value)} aria-describedby={error ? 'login-error' : undefined} />
					</div>
					<PasswordField id="password" label={tr('Password', 'Senha')} autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} aria-describedby={error ? 'login-error' : undefined} />
					<p id="login-error" role="alert" className="min-h-5 text-sm text-destructive">{error}</p>
					<Button type="submit" className="w-full" disabled={signIn.isPending}>
						{signIn.isPending ? tr('Signing in…', 'Entrando…') : tr('Sign in', 'Entrar')}
					</Button>
				</form>
				<div className="flex justify-center">
					<Button variant="ghost" onClick={() => setLocale(otherLocale)} aria-label={tr(`Switch language to ${LOCALE_LABELS[otherLocale]}`, `Mudar idioma para ${LOCALE_LABELS[otherLocale]}`)}>
						{LOCALE_LABELS[otherLocale]}
					</Button>
				</div>
			</div>
		</main>
	);
}
