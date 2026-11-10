import { Copy, Minus, Square, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { BrandMark } from '@/components/brand';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';

type WindowAction = 'minimize' | 'toggleMaximize' | 'close';

/** Window controls for the frameless desktop window; the server build never renders it. */
export function DesktopTitlebar() {
	const [maximized, setMaximized] = useState(false);
	const [error, setError] = useState('');

	useEffect(() => {
		if (__LEARNVAULT_TRANSPORT__ !== 'desktop') return;
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void (async () => {
			try {
				const { getCurrentWindow } = await import('@tauri-apps/api/window');
				const appWindow = getCurrentWindow();
				const sync = async () => setMaximized(await appWindow.isMaximized());
				await sync();
				const stop = await appWindow.onResized(sync);
				if (disposed) stop();
				else unlisten = stop;
			} catch (cause) {
				if (!disposed) setError(tr(`Window controls are unavailable: ${String(cause)}`, `Os controles da janela não estão disponíveis: ${String(cause)}`));
			}
		})();
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, []);

	const perform = async (action: WindowAction) => {
		try {
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			const appWindow = getCurrentWindow();
			if (action === 'minimize') await appWindow.minimize();
			else if (action === 'close') await appWindow.close();
			else {
				await appWindow.toggleMaximize();
				setMaximized(await appWindow.isMaximized());
			}
		} catch (cause) {
			setError(tr(`The window action failed: ${String(cause)}`, `A ação da janela falhou: ${String(cause)}`));
		}
	};

	if (__LEARNVAULT_TRANSPORT__ !== 'desktop') return null;
	return (
		<div data-tauri-drag-region className="flex h-11 items-center gap-2 border-b bg-muted/40 pl-3 select-none">
			<BrandMark className="size-5 text-primary" />
			<span data-tauri-drag-region className="text-sm font-medium">LearnVault</span>
			{error && <span role="alert" className="truncate text-xs text-destructive">{error}</span>}
			<div role="group" aria-label={tr('Window controls', 'Controles da janela')} className="ml-auto flex">
				<Button variant="ghost" size="icon" onClick={() => void perform('minimize')} aria-label={tr('Minimize', 'Minimizar')}><Minus /></Button>
				<Button variant="ghost" size="icon" onClick={() => void perform('toggleMaximize')} aria-label={maximized ? tr('Restore', 'Restaurar') : tr('Maximize', 'Maximizar')}>{maximized ? <Copy /> : <Square />}</Button>
				<Button variant="ghost" size="icon" onClick={() => void perform('close')} aria-label={tr('Close', 'Fechar')} className="hover:bg-destructive hover:text-destructive-foreground"><X /></Button>
			</div>
		</div>
	);
}
