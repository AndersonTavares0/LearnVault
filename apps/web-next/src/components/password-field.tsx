import { Eye, EyeOff } from 'lucide-react';
import { useState, type ComponentProps } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

/** Password input with a show/hide toggle; paste stays allowed (WCAG 3.3.8). */
export function PasswordField({ id, label, labelClassName, className, ...props }: { id: string; label: string; labelClassName?: string } & Omit<ComponentProps<typeof Input>, 'id' | 'type'>) {
	const [visible, setVisible] = useState(false);
	return (
		<div className="space-y-2">
			<Label htmlFor={id} className={labelClassName}>{label}</Label>
			<div className="relative">
				<Input id={id} name={id} type={visible ? 'text' : 'password'} className={cn('pr-12', className)} {...props} />
				<Button type="button" variant="ghost" size="icon" className="absolute top-0 right-0" aria-label={visible ? tr('Hide password', 'Ocultar senha') : tr('Show password', 'Mostrar senha')} aria-pressed={visible} onClick={() => setVisible((value) => !value)}>
					{visible ? <EyeOff /> : <Eye />}
				</Button>
			</div>
		</div>
	);
}
