import { Link, useRouteError } from 'react-router';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { tr } from '@/lib/i18n';

/** Route-level error boundary: one broken page must not blank the whole app. */
export function RouteError() {
	useLocale();
	const error = useRouteError();
	// Details go to the console only; the page never renders raw error text.
	console.error(error);
	return (
		<main id="main" role="alert" className="mx-auto flex min-h-dvh w-full max-w-xl flex-col items-start justify-center gap-4 px-4 py-12">
			<h1 className="text-3xl font-semibold text-balance">{tr('This page hit a problem', 'Esta página teve um problema')}</h1>
			<p className="text-muted-foreground">{tr('Your courses and progress are safe. Reload the page, or go back to your library.', 'Seus cursos e progresso estão seguros. Recarregue a página ou volte para a biblioteca.')}</p>
			<div className="flex flex-wrap gap-2">
				<Button onClick={() => window.location.reload()}>{tr('Reload', 'Recarregar')}</Button>
				<Button variant="outline" render={<Link to="/" />}>{tr('Back to my learning', 'Voltar ao meu aprendizado')}</Button>
			</div>
		</main>
	);
}
