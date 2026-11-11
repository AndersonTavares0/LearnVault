import { ArrowRight, Pin, Search, type LucideIcon } from 'lucide-react';
import { useMemo, useState, type ReactNode } from 'react';
import { Link } from 'react-router';
import { courseTone } from '@/components/cover-art';
import { Eyebrow } from '@/components/panel';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { courseCode } from '@/lib/cover';
import { tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry } from '@/lib/library';
import { normalizeForSearch } from '@/lib/search';
import { useCatalog, useFavorites, useRecent, useToggleCourseFavorite } from '@/app/data';
import { PageHeader } from '@/app/shell';

/** The id of the group of items whose lessons are no longer in the library. */
export const GONE = '__gone';

type Props = {
	icon: LucideIcon;
	title: string;
	description?: string;
	actions?: ReactNode;
	/** What the card's footer says for a course, e.g. "3 notes" or "2 due". */
	footer: (courseId: string) => string;
	/** Where a course leads. */
	to: (courseId: string) => string;
	/** Shown after the courses when some items belong to lessons that are gone. */
	hasGone?: boolean;
	goneTitle?: string;
	empty?: string;
};

/**
 * Start from a course: a title, the pinned (favourite) courses, then all of them with a search.
 * Each card has the course code and a tinted footer that says how much is waiting inside.
 */
export function CoursePicker({ icon, title, description, actions, footer, to, hasGone, goneTitle, empty }: Props) {
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const [query, setQuery] = useState('');
	const favoriteIds = useMemo(() => new Set(favorites.data?.courseIds ?? []), [favorites.data]);
	const entries = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: favoriteIds, query: '', filter: 'all', category: null }).entries, [catalog.data, recent.data, favoriteIds]);
	const needle = normalizeForSearch(query.trim());
	const shown = entries.filter((entry) => !needle || normalizeForSearch(entry.course.title).includes(needle));
	const pinned = entries.filter((entry) => entry.favorite);
	const showGone = hasGone && !needle;

	return (
		<div className="space-y-10">
			<PageHeader icon={icon} title={title} description={description} actions={actions} />
			<section aria-labelledby="pinned-heading" className="space-y-3">
				<Eyebrow className="flex items-center gap-2"><Pin className="size-3.5" aria-hidden="true" /><span id="pinned-heading">{tr('Your courses', 'Seus cursos')}</span></Eyebrow>
				{pinned.length === 0 ? (
					<p className="rounded-[1.75rem] border border-dashed p-6 text-center text-sm text-muted-foreground">{tr('No pinned courses yet. Pin any course below to keep it here.', 'Nenhum curso fixado ainda. Fixe qualquer curso abaixo para mantê-lo aqui.')}</p>
				) : (
					<ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">{pinned.map((entry) => <PickerCard key={entry.courseId} entry={entry} footer={footer(entry.courseId)} to={to(entry.courseId)} />)}</ul>
				)}
			</section>
			<section aria-labelledby="all-heading" className="space-y-3">
				<div className="flex flex-wrap items-center justify-between gap-3">
					<Eyebrow><span id="all-heading">{tr('All courses', 'Todos os cursos')}</span></Eyebrow>
					<div role="search" aria-label={title} className="relative w-full sm:w-64">
						<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
						<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search courses…', 'Buscar cursos…')} aria-label={tr('Filter courses', 'Filtrar cursos')} className="pl-9" />
					</div>
				</div>
				{shown.length === 0 && !showGone ? (
					<p role="status" className="text-muted-foreground">{entries.length === 0 ? empty ?? tr('No courses yet.', 'Ainda não há cursos.') : tr('No courses match your search.', 'Nenhum curso corresponde à busca.')}</p>
				) : (
					<ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
						{shown.map((entry) => <PickerCard key={entry.courseId} entry={entry} footer={footer(entry.courseId)} to={to(entry.courseId)} />)}
						{showGone && <PickerCard title={goneTitle ?? tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca')} footer={footer(GONE)} to={to(GONE)} />}
					</ul>
				)}
			</section>
		</div>
	);
}

function PickerCard({ entry, title: given, footer, to }: { entry?: CourseEntry; title?: string; footer: string; to: string }) {
	const toggle = useToggleCourseFavorite();
	const title = entry?.course.title ?? given ?? '';
	const tone = courseTone(title);
	return (
		<li className="group/pick relative overflow-hidden rounded-[1.5rem] border bg-card transition-shadow focus-within:ring-3 focus-within:ring-ring hover:shadow-md">
			<div className="flex items-center gap-3 p-4 pb-2">
				<span className="grid size-9 place-items-center rounded-xl font-heading text-sm font-bold" style={{ backgroundColor: tone.tint, color: tone.ink }} aria-hidden="true">{title.trim()[0]?.toUpperCase()}</span>
				<span className="text-sm font-bold tabular-nums" style={{ color: tone.ink }}>{courseCode(title)}</span>
				{entry && (
					<Button variant="ghost" size="icon" className="relative z-10 ml-auto size-11" aria-pressed={entry.favorite} disabled={toggle.isPending} onClick={() => toggle.mutate({ courseId: entry.courseId, favorite: !entry.favorite })} aria-label={entry.favorite ? tr(`Unpin ${title}`, `Desafixar ${title}`) : tr(`Pin ${title}`, `Fixar ${title}`)}>
						<Pin className={entry.favorite ? 'fill-current' : ''} />
					</Button>
				)}
			</div>
			<h3 className="line-clamp-2 min-h-[4.25rem] px-4 pb-4 font-heading text-lg leading-snug font-bold">
				<Link to={to} className="after:absolute after:inset-0 after:content-[''] focus-visible:outline-none">{title}<span className="sr-only">: {footer}</span></Link>
			</h3>
			<div className="flex items-center justify-between px-4 py-2.5 text-xs font-bold" style={{ backgroundColor: tone.tint, color: tone.ink }} aria-hidden="true">
				<span>{footer}</span><ArrowRight className="size-4 transition-transform group-hover/pick:translate-x-0.5" />
			</div>
		</li>
	);
}
