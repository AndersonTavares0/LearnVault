/** FNV-1a: the same title always gets the same ground. */
function hash(value: string): number {
	let h = 0x811c9dc5;
	for (let i = 0; i < value.length; i += 1) {
		h ^= value.charCodeAt(i);
		h = Math.imul(h, 0x01000193);
	}
	return h >>> 0;
}

// Course colours: muted dark grounds, separate from the brand palette so two courses never look
// alike in any palette. Every ground keeps white text above 7:1.
const GROUNDS = ['#14342b', '#1e1b4b', '#3b0d2c', '#0f2f3a', '#3a1d0e', '#1c1917', '#172554', '#14532d'];

const LINE_CHARS = 13;
const MAX_LINES = 3;
/** Above this many lessons one bar per lesson stops being readable, so the bar becomes continuous. */
const MAX_SEGMENTS = 40;

/** Greedy word wrap for the SVG title; the last visible line gets an ellipsis if text is left over. */
export function wrapTitle(title: string): string[] {
	const lines: string[] = [];
	let line = '';
	for (const word of title.split(/\s+/).filter(Boolean)) {
		if (line && line.length + 1 + word.length > LINE_CHARS) {
			lines.push(line);
			line = word;
		} else {
			line = line ? `${line} ${word}` : word;
		}
	}
	if (line) lines.push(line);
	if (lines.length <= MAX_LINES) return lines;
	const kept = lines.slice(0, MAX_LINES);
	kept[MAX_LINES - 1] = `${kept[MAX_LINES - 1].slice(0, LINE_CHARS - 1)}…`;
	return kept;
}

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
			className={empty ? 'fill-white/10' : index < finished ? 'fill-white' : index === finished ? 'fill-[var(--reward-fill)]' : 'fill-white/15'}
		/>
	));
}

type Props = {
	title: string;
	/** Lessons finished and total lessons; the bar under the title is the course drawn as its own lessons. */
	done: number;
	total: number;
	/** Small thumbnails show the first letter instead of the title. */
	compact?: boolean;
	/** Draws one block per lesson instead of the title, for places where the name is already the heading. */
	map?: boolean;
};

/**
 * Cover for a course that has no uploaded image and no video thumbnail (ADR 0002): the course
 * name, set like a book jacket, and one bar per lesson, lit as lessons are finished. Decorative;
 * the text next to it names the course.
 */
export function CoverArt({ title, done, total, compact = false, map = false }: Props) {
	const ground = GROUNDS[hash(title.normalize('NFD').toLowerCase()) % GROUNDS.length];
	const lines = wrapTitle(title);
	const size = Math.max(...lines.map((line) => line.length), 0) <= LINE_CHARS - 1 ? 15 : 13;
	const finished = Math.min(done, total);
	const segments = total > 0 && total <= MAX_SEGMENTS ? total : 0;
	const gap = total <= 14 ? 2 : 0.6;
	const width = segments ? (140 - gap * (segments - 1)) / segments : 0;
	if (map) return <svg viewBox="0 0 160 100" aria-hidden="true" focusable="false" className="absolute inset-0 size-full" preserveAspectRatio="xMidYMid slice"><rect width="160" height="100" fill={ground} />{lessonMap(done, total)}</svg>;
	return (
		<svg viewBox="0 0 160 100" aria-hidden="true" focusable="false" className="absolute inset-0 size-full" preserveAspectRatio="xMidYMid slice">
			<rect width="160" height="100" fill={ground} />
			<g className="cover-paper">
			{compact ? (
				<text x="12" y="66" fontSize="56" className="fill-white font-serif font-bold">{(title.trim()[0] ?? '?').toUpperCase()}</text>
			) : (
				<>
					<rect x="12" y="20" width="22" height="2" rx="1" className="fill-[var(--reward-fill)]" />
					{lines.map((line, index) => (
						<text key={index} x="12" y={38 + index * (size + 3)} fontSize={size} className="fill-white font-serif font-bold">{line}</text>
					))}
				</>
			)}
			{segments > 0 && Array.from({ length: segments }, (_, index) => (
				<rect
					key={index}
					x={10 + index * (width + gap)}
					y="86"
					width={width}
					height="4"
					rx="2"
					className={index < finished ? 'fill-white' : index === finished ? 'fill-[var(--reward-fill)]' : 'fill-white/20'}
				/>
			))}
			{total > MAX_SEGMENTS && (
				<>
					<rect x="10" y="86" width="140" height="4" rx="2" className="fill-white/20" />
					<rect x="10" y="86" width={(140 * finished) / total} height="4" rx="2" className="fill-white" />
				</>
			)}
			</g>
		</svg>
	);
}
