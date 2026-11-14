import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AlertDialog, AlertDialogContent, AlertDialogDescription, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { setLocale } from '@/lib/i18n';

afterEach(() => {
	cleanup();
	setLocale('en');
});

const open = () => render(
	<Dialog open>
		<DialogContent>
			<DialogHeader><DialogTitle>Title</DialogTitle><DialogDescription>Text</DialogDescription></DialogHeader>
		</DialogContent>
	</Dialog>
);

describe('Dialog', () => {
	it('names its close button in the current language', () => {
		open();
		expect(screen.getByRole('button', { name: 'Close' })).toBeTruthy();
		cleanup();
		setLocale('pt-BR');
		open();
		expect(screen.getByRole('button', { name: 'Fechar' })).toBeTruthy();
	});

	it('never grows past the viewport and scrolls inside it', () => {
		open();
		const popup = screen.getByRole('dialog');
		expect(popup.className).toContain('max-h-[calc(100dvh-2rem)]');
		expect(popup.className).toContain('overflow-y-auto');
	});

	it('keeps confirmation dialogs inside the viewport too', () => {
		render(
			<AlertDialog open>
				<AlertDialogContent>
					<AlertDialogHeader><AlertDialogTitle>Sure?</AlertDialogTitle><AlertDialogDescription>Text</AlertDialogDescription></AlertDialogHeader>
				</AlertDialogContent>
			</AlertDialog>
		);
		const popup = screen.getByRole('alertdialog');
		expect(popup.className).toContain('max-h-[calc(100dvh-2rem)]');
		expect(popup.className).toContain('overflow-y-auto');
	});
});
