import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { tr } from '@/lib/i18n';

/** Confirms an action that cannot be undone; the dialog primitive traps focus and Escape closes it. */
export function ConfirmDelete({ open, onOpenChange, title, description, label, pending, onConfirm }: {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	title: string;
	description: string;
	label: string;
	pending?: boolean;
	onConfirm: () => void;
}) {
	return (
		<AlertDialog open={open} onOpenChange={onOpenChange}>
			<AlertDialogContent>
				<AlertDialogHeader>
					<AlertDialogTitle>{title}</AlertDialogTitle>
					<AlertDialogDescription>{description}</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter>
					<AlertDialogCancel>{tr('Cancel', 'Cancelar')}</AlertDialogCancel>
					<AlertDialogAction variant="destructive" disabled={pending} onClick={onConfirm}>{label}</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
