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

// Calm, low-chroma grounds for a dark title card. Amber is kept for the current lesson, so no warm yellow here.
const FRAME_HUES = [25, 55, 145, 175, 215, 265, 320];
// Lecture frames are always dark, in both app themes, so the library reads as one set of videos.
const BONE = '#ECE7DA';

/** One block per lesson in rows, the finished ones lit and the next one marked. */
function lessonMap(done: number, total: number, hue: number) {
	// A course with no playable lessons yet still gets a cover: a few dim blocks.
	const empty = total <= 0;
	const shown = empty ? 6 : Math.min(total, 60);
	const columns = Math.min(shown, shown > 24 ? 12 : 8);
	const rows = Math.ceil(shown / columns);
	const gap = 3;
	const width = (140 - gap * (columns - 1)) / columns;
	const height = Math.min(26, (76 - gap * (rows - 1)) / rows);
	// Centered, so a short course does not leave the map stuck to the top.
	const top = (100 - (rows * height + gap * (rows - 1))) / 2;
	const finished = Math.min(done, total);
	return Array.from({ length: shown }, (_, index) => (
		<rect
			key={index}
			x={10 + (index % columns) * (width + gap)}
			y={top + Math.floor(index / columns) * (height + gap)}
			width={width}
			height={height}
			rx="3"
			fill={index === finished && !empty ? `oklch(0.9 0.05 ${hue})` : BONE}
			fillOpacity={empty ? 0.12 : index < finished ? 0.9 : index === finished ? 1 : 0.18}
		/>
	));
}

type Props = {
	title: string;
	courseId?: string;
	/** Lessons finished and total lessons; the map draws the course as its own lessons. */
	done: number;
	total: number;
	/** Small thumbnails show only the first letter on the ground. */
	compact?: boolean;
	/** Draws one block per lesson instead of the title, for places where the name is already the heading. */
	map?: boolean;
};

/**
 * Generated cover for a course with no uploaded image and no video frame (ADR 0002): a dark lecture
 * title card, the opening frame of a course video, with the title in bone and a short rule under it.
 * The ground hue is stable per course. Decorative: the card beside it carries the name and progress.
 */
export function CoverArt({ title, courseId, done, total, compact = false, map = false }: Props) {
	const hue = FRAME_HUES[hash(courseId || title.normalize('NFD').toLowerCase()) % FRAME_HUES.length];
	const ground: CSSProperties = { background: `radial-gradient(120% 100% at 0% 0%, oklch(0.31 0.04 ${hue}) 0%, transparent 60%), oklch(0.24 0.035 ${hue})` };
	const letter = courseMonogram(title).charAt(0);
	const lessons = total === 1 ? tr('1 lesson', '1 aula') : tr(`${total} lessons`, `${total} aulas`);
	return (
		<div aria-hidden="true" className="absolute inset-0 @container overflow-hidden" style={ground}>
			{map && <svg viewBox="0 0 160 100" focusable="false" className="absolute inset-0 size-full" preserveAspectRatio="xMidYMid slice">{lessonMap(done, total, hue)}</svg>}
			{!map && compact && <span className="absolute inset-0 grid place-items-center font-heading text-sm font-semibold" style={{ color: BONE }}>{letter}</span>}
			{!map && !compact && (
				<div className="absolute inset-0 flex flex-col justify-center" style={{ padding: '8cqi' }}>
					<p className="shrink-0 line-clamp-3 text-balance font-heading font-semibold tracking-tight" style={{ color: BONE, fontSize: 'clamp(14px, 10cqi, 38px)', lineHeight: 1.2, paddingBottom: '0.06em' }}>{title}</p>
					<span className="block shrink-0" style={{ marginTop: '5cqi', width: '12cqi', minWidth: 24, height: 3, background: `oklch(0.84 0.05 ${hue})` }} />
					{total > 0 && <p className="font-medium" style={{ marginTop: '3cqi', color: 'rgba(236, 231, 218, 0.62)', fontSize: 'clamp(11px, 4cqi, 14px)' }}>{lessons}</p>}
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
