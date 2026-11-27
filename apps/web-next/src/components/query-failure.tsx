import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';

export function QueryFailure({ message, retry }: { message: string; retry: () => void }) {
	return <div className="space-y-3">
		<p role="alert" className="text-destructive">{message}</p>
		<Button variant="outline" onClick={retry}>{tr('Try again', 'Tentar novamente')}</Button>
	</div>;
}
