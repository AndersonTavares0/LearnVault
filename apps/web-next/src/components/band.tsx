import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

/**
 * The dark title band at the top of a page, like the ones course platforms use: it runs edge to
 * edge while the content below stays in the page column. Its colour follows the learner's choice
 * of "Banners" in Settings.
 */
export function Band({ children, className }: { children: ReactNode; className?: string }) {
	return (
		<div className={cn('relative left-1/2 -mt-6 mb-8 w-screen -translate-x-1/2 bg-ink text-ink-foreground', className)}>
			<div className="mx-auto max-w-7xl px-4 pt-8">{children}</div>
		</div>
	);
}
