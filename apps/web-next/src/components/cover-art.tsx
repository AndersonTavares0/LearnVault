import { courseMonogram } from '@/lib/cover';

/** FNV-1a: the same title always gets the same ground. */
function hash(value: string): number {
	let h = 0x811c9dc5;
	for (let i = 0; i < value.length; i += 1) {
		h ^= value.charCodeAt(i);
		h = Math.imul(h, 0x01000193);
	}
	return h >>> 0;
}

// Course colours: pale warm papers that sit on the cream page, one per title. Dark brown text on
// every one stays above 7:1.
const GROUNDS = ['#F6E7CF', '#F2E0D2', '#EAE4CF', '#E3E8D6', '#E9DED0', '#F1DCD3', '#E6E3D8', '#EFE2C8'];

/** One block per lesson in rows, the finished ones lit and the next one marked. */
function lessonMap(done: number, total: number) {
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
			className={empty ? 'fill-foreground/10' : index < finished ? 'fill-primary' : index === finished ? 'fill-[var(--reward-fill)]' : 'fill-foreground/15'}
		/>
	));
}

type Props = {
	title: string;
	/** Lessons finished and total lessons; the map draws the course as its own lessons. */
	done: number;
	total: number;
	/** Small thumbnails show the first letter instead of the initials. */
	compact?: boolean;
	/** Draws one block per lesson instead of the initials, for places where the name is already the heading. */
	map?: boolean;
};

/**
 * Cover for a course that has no uploaded image and no video thumbnail (ADR 0002): a pale paper
 * with the course's initials set large, like a book jacket. It carries no title and no progress,
 * because the card beside it already shows both. Decorative.
 */
export function CoverArt({ title, done, total, compact = false, map = false }: Props) {
	const ground = GROUNDS[hash(title.normalize('NFD').toLowerCase()) % GROUNDS.length];
	if (map) return <svg viewBox="0 0 160 100" aria-hidden="true" focusable="false" className="absolute inset-0 size-full" preserveAspectRatio="xMidYMid slice"><rect width="160" height="100" fill={ground} />{lessonMap(done, total)}</svg>;
	const letters = compact ? (title.trim()[0] ?? '?').toUpperCase() : courseMonogram(title);
	return (
		<svg viewBox="0 0 160 100" aria-hidden="true" focusable="false" className="absolute inset-0 size-full" preserveAspectRatio="xMidYMid slice">
			<rect width="160" height="100" fill={ground} />
			<rect x="14" y="18" width="22" height="2.5" rx="1.25" className="fill-[var(--reward-fill)]" />
			<text x="12" y="80" fontSize={compact ? 56 : 52} className="fill-foreground font-heading font-bold">{letters}</text>
		</svg>
	);
}

const TONE_HUES = [38, 95, 150, 175, 225, 262, 330, 20];

/** A soft tint and a dark ink of one hue per title, for small coloured labels (ink on white is above 7:1). */
export function courseTone(title: string): { tint: string; ink: string } {
	const hue = TONE_HUES[hash(title.normalize('NFD').toLowerCase()) % TONE_HUES.length];
	return { tint: `hsl(${hue} 45% 92%)`, ink: `hsl(${hue} 60% 22%)` };
}
