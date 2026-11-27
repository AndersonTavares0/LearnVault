import type { CSSProperties } from 'react';
import { courseMonogram } from '@/lib/cover';
import { tr } from '@/lib/i18n';

/** FNV-1a: the same title always gets the same ground. */
function hash(value: string): number {
	let h = 0x811c9dc5;
	for (let i = 0; i < value.length; i += 1) {
		h ^= value.charCodeAt(i);
		h = Math.imul(h, 0x01000193);
	}
	return h >>> 0;
}

// Calm, harmonious hues for generated course plates.
const FRAME_HUES = [165, 195, 220, 145, 260, 35, 325];

/** One block per lesson in rows, the finished ones lit in the primary accent and the next one ringed. */
function lessonMap(done: number, total: number) {
	// A course with no playable lessons yet still gets a structured architectural plate.
	const empty = total <= 0;
	const shown = empty ? 8 : Math.min(total, 60);
	const columns = Math.min(shown, shown > 24 ? 12 : 6);
	const rows = Math.ceil(shown / columns);
	const gap = 4;
	const width = (140 - gap * (columns - 1)) / columns;
	const height = Math.min(24, (74 - gap * (rows - 1)) / rows);
	const top = (100 - (rows * height + gap * (rows - 1))) / 2;
	const finished = Math.min(done, total);
	return Array.from({ length: shown }, (_, index) => {
		const isDone = !empty && index < finished;
		const isCurrent = !empty && finished > 0 && index === finished;
		return (
			<rect
				key={index}
				x={10 + (index % columns) * (width + gap)}
				y={top + Math.floor(index / columns) * (height + gap)}
				width={width}
				height={height}
				rx="3"
				fill={isDone ? 'var(--primary)' : 'currentColor'}
				fillOpacity={isDone ? 0.92 : isCurrent ? 0.24 : 0.14}
				stroke={isCurrent ? 'var(--primary)' : 'currentColor'}
				strokeOpacity={isCurrent ? 0.95 : 0.18}
				strokeWidth={isCurrent ? 1.5 : 0.75}
			/>
		);
	});
}

type Props = {
	title: string;
	courseId?: string;
	/** Lessons finished and total lessons; the map draws the course as its own lessons. */
	done: number;
	total: number;
	/** Small thumbnails show only the monogram on the paper/slate plate. */
	compact?: boolean;
	/** Draws one block per lesson instead of the title, for places where the name is already the heading. */
	map?: boolean;
};

/**
 * Generated cover for a course with no uploaded image and no video frame (ADR 0002): a surface-aware
 * studio plate matching Light paper or Dark slate, with crisp monogram or lesson-map geometry.
 */
export function CoverArt({ title, courseId, done, total, compact = false, map = false }: Props) {
	const hue = FRAME_HUES[hash(courseId || title.normalize('NFD').toLowerCase()) % FRAME_HUES.length];
	const ground = { '--cover-hue': hue } as CSSProperties;
	const letter = courseMonogram(title).charAt(0);
	const lessons = total === 1 ? tr('1 lesson', '1 aula') : tr(`${total} lessons`, `${total} aulas`);
	return (
		<div aria-hidden="true" className="cover-plate absolute inset-0 @container overflow-hidden" style={ground}>
			{map && (
				<svg viewBox="0 0 160 100" focusable="false" className="pointer-events-none absolute inset-0 size-full" preserveAspectRatio="xMidYMid slice">
					<path d="M0 78 L160 32 M0 96 L160 50" stroke="var(--cover-rule)" strokeWidth="1" fill="none" />
					{lessonMap(done, total)}
				</svg>
			)}
			{!map && compact && (
				<span className="absolute inset-0 grid place-items-center font-heading text-base font-bold tracking-tight" style={{ color: 'var(--cover-ink)' }}>{letter}</span>
			)}
			{!map && !compact && (
				<div className="absolute inset-0 flex flex-col justify-center" style={{ padding: '8cqi' }}>
					<p className="shrink-0 line-clamp-3 text-balance font-heading font-semibold tracking-tight" style={{ color: 'var(--cover-ink)', fontSize: 'clamp(11px, 9.5cqi, 28px)', lineHeight: 1.18, paddingBottom: '0.06em' }}>{title}</p>
					{total > 0 && <p className="font-medium" style={{ marginTop: '2.5cqi', color: 'var(--cover-sub)', fontSize: 'clamp(11px, 4cqi, 13px)' }}>{lessons}</p>}
				</div>
			)}
		</div>
	);
}

const TONE_HUES = [38, 95, 150, 175, 225, 262, 330, 20];

/** The hue of a course's small labels, as a custom property the `.tone` class reads. */
export function courseTone(title: string): CSSProperties {
	return { '--hue': TONE_HUES[hash(title.normalize('NFD').toLowerCase()) % TONE_HUES.length] } as CSSProperties;
}
