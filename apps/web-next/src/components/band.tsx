import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

/** The title block at the top of a page: a serif title over a hairline, no coloured bar. */
export function Band({ children, className }: { children: ReactNode; className?: string }) {
	return <div className={cn('mb-10 border-b pb-6', className)}>{children}</div>;
}
