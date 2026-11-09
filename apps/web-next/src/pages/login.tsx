import { useState, type FormEvent } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router';
import { BrandMark } from '@/components/brand';
import { PasswordField } from '@/components/password-field';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { LOCALE_LABELS, setLocale, tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import { useSession, useSignIn } from '@/app/data';

// Fields are ruled lines on a library card: serif values, mono labels. The ring stays for focus.
const field = 'rounded-none border-0 border-b-[1.5px] border-foreground bg-transparent px-0 font-serif text-2xl md:text-2xl dark:bg-transparent';
const fieldLabel = 'font-mono text-xs tracking-widest text-muted-foreground uppercase';

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
			<div className="w-full max-w-md space-y-6">
				{/* Light: a dashed library card with a folded corner. Dark: the card chrome drops away. */}
				<form onSubmit={submit} className="relative space-y-6 rounded-lg border-[1.5px] border-dashed border-muted-foreground/50 bg-card p-8 dark:border-transparent dark:bg-transparent" noValidate={false}>
					<span aria-hidden="true" className="absolute -top-px -left-px size-0 rounded-tl-lg border-t-[26px] border-r-[26px] border-t-primary border-r-transparent dark:hidden" />
					<div className="flex items-center justify-between gap-3">
						<div className="flex items-center gap-3">
							<BrandMark className="size-9 text-primary" />
							<h1 className="font-serif text-xl font-bold tracking-tight sm:text-2xl">{tr('Sign in to LearnVault', 'Entrar no LearnVault')}</h1>
						</div>
						<span aria-hidden="true" className="hidden font-mono text-[0.7rem] tracking-widest whitespace-nowrap text-muted-foreground uppercase md:inline dark:hidden">{tr('Study card', 'Ficha de estudo')}</span>
					</div>
					<div className="space-y-1.5">
						<Label htmlFor="username" className={fieldLabel}>{tr('Username', 'Usuário')}</Label>
						<Input id="username" name="username" autoComplete="username" autoCapitalize="none" spellCheck={false} required value={username} onChange={(event) => setUsername(event.target.value)} aria-describedby={error ? 'login-error' : undefined} className={cn(field)} />
					</div>
					<PasswordField id="password" label={tr('Password', 'Senha')} labelClassName={fieldLabel} className={field} autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} aria-describedby={error ? 'login-error' : undefined} />
					<p id="login-error" role="alert" className="min-h-5 text-sm text-destructive">{error}</p>
					<Button type="submit" size="lg" className="w-full" disabled={signIn.isPending}>
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
