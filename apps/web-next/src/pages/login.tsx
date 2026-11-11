import { useState, type FormEvent } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router';
import { CircleAlert } from 'lucide-react';
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
		<main id="main" className="grid min-h-dvh place-items-center bg-muted px-4 py-10">
			<div className="flex w-full max-w-md flex-col items-center gap-6">
				<div className="flex items-center gap-3 text-2xl font-bold"><BrandMark className="size-9 text-primary" />LearnVault</div>
				<form onSubmit={submit} className="w-full space-y-5 rounded-lg border bg-card p-8 shadow-sm">
					<div className="space-y-1">
						<h1 className="text-2xl font-bold">{tr('Sign in', 'Entrar')}</h1>
						<p className="text-muted-foreground">{tr('Welcome back. Your courses are where you left them.', 'Bem-vindo de volta. Seus cursos estão onde você deixou.')}</p>
					</div>
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
				<p className="text-center text-sm text-muted-foreground">{tr('Accounts are created by the administrator. Nothing leaves this server.', 'As contas são criadas pelo administrador. Nada sai deste servidor.')}</p>
				<Button variant="ghost" onClick={() => setLocale(otherLocale)} aria-label={tr(`Switch language to ${LOCALE_LABELS[otherLocale]}`, `Mudar idioma para ${LOCALE_LABELS[otherLocale]}`)}>
					{LOCALE_LABELS[otherLocale]}
				</Button>
			</div>
		</main>
	);
}
