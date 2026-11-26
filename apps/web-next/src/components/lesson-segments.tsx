import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

/** Past this many lessons one segment per lesson gets too thin to read, so the line becomes one bar. */
const MAX_SEGMENTS = 40;

export type SegmentState = 'done' | 'current' | 'pending';

/**
 * Course progress drawn one segment per lesson. State is carried by the segment's form as well as its
 * colour: done lessons are tall, the current one is half filled, and lessons not started are a hairline.
 * Pass `states` (one per lesson, in course order) when known; otherwise the first `done` lessons count as done
 * and `current` is the zero-based index of the lesson to resume.
 */
export function LessonSegments({ done, total, current, states, className }: { done: number; total: number; current?: number; states?: readonly SegmentState[]; className?: string }) {
	const label = tr(`${done} of ${total} lessons done`, `${done} de ${total} aulas concluídas`);
	const shared = { role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': total, 'aria-valuenow': done, 'aria-label': label } as const;
	if (total <= 0) return null;
	if (total > MAX_SEGMENTS) {
		return (
			<div {...shared} className={cn('flex h-2 items-end', className)}>
				<div className="h-0.5 w-full rounded-full bg-[var(--border-strong)]">
					<div className="h-1.5 -translate-y-1 rounded-full bg-success" style={{ width: `${(done / total) * 100}%` }} />
				</div>
			</div>
		);
	}
	return (
		<div {...shared} className={cn('flex h-2 items-end gap-[3px]', className)}>
			{Array.from({ length: total }, (_, index) => {
				const state: SegmentState = states?.[index] ?? (index === current ? 'current' : index < done ? 'done' : 'pending');
				return (
					<span
						key={index}
						data-state={state}
						className={cn('flex-1 rounded-full', state === 'done' ? 'h-1.5 bg-success' : state === 'current' ? 'h-1.5 bg-[linear-gradient(90deg,var(--brand)_45%,var(--border-strong)_45%)]' : 'h-0.5 bg-[var(--border-strong)]')}
					/>
				);
			})}
		</div>
	);
}
