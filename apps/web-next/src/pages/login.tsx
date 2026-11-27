import { useState, type FormEvent } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router';
import { CircleAlert, Info } from 'lucide-react';
import { Motto } from '@/components/brand';
import { PasswordField } from '@/components/password-field';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { LOCALE_LABELS, setLocale, tr } from '@/lib/i18n';
import { sessionExpiredNotice } from '@/lib/session-guard';
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
	const expired = location.state?.sessionExpired === true;
	return (
		<main id="main" className="grid min-h-dvh place-items-center bg-background px-4 py-10">
			<div className="flex w-full max-w-sm flex-col gap-6">
				<div className="study-surface p-7 shadow-xs sm:p-9">
					<p className="mb-7 text-center font-heading text-2xl font-semibold tracking-tight">LearnVault</p>
					<form onSubmit={submit} className="space-y-5">
						<h1 className="text-center font-heading text-2xl font-semibold">{tr('Sign in', 'Entrar')}</h1>
						{expired && !error && (
							<p role="status" className="flex items-start gap-3 rounded-lg bg-info-tint p-3 text-sm text-info">
								<Info aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
								{sessionExpiredNotice()}
							</p>
						)}
						{error && (
							<p id="login-error" role="alert" className="flex items-start gap-3 rounded-lg border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
								<CircleAlert aria-hidden="true" className="mt-0.5 size-5 shrink-0" />
								<span>{error}</span>
							</p>
						)}
						<div className="space-y-1.5">
							<Label htmlFor="username">{tr('Username', 'Usuário')}</Label>
							<Input id="username" name="username" autoComplete="username" autoCapitalize="none" spellCheck={false} required value={username} onChange={(event) => setUsername(event.target.value)} aria-describedby={error ? 'login-error' : undefined} />
						</div>
						<PasswordField id="password" label={tr('Password', 'Senha')} autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} aria-describedby={error ? 'login-error' : undefined} />
						<Button type="submit" size="lg" className="w-full" disabled={signIn.isPending}>
							{signIn.isPending ? tr('Signing in…', 'Entrando…') : tr('Sign in', 'Entrar')}
						</Button>
					</form>
				</div>
				<Motto name="sapereAude" className="text-center text-foreground/75" />
				<Button variant="outline" size="sm" className="mx-auto" onClick={() => setLocale(otherLocale)} aria-label={tr(`Switch language to ${LOCALE_LABELS[otherLocale]}`, `Mudar idioma para ${LOCALE_LABELS[otherLocale]}`)}>
					{LOCALE_LABELS[otherLocale]}
				</Button>
			</div>
		</main>
	);
}
