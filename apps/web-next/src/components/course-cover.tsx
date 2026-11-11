import { useEffect, useRef, useState } from 'react';
import { CoverArt } from '@/components/cover-art';
import { selectCourseCoverSource } from '@/lib/cover';
import { IndexedDbThumbnailCacheAdapter, loadCourseThumbnail, observeThumbnailVisibility, ThumbnailCache, ThumbnailTaskQueue } from '@/lib/thumbnails';
import { courseIdOf, transport, type CatalogNode } from '@/lib/transport';
import { cn } from '@/lib/utils';

// One cache and one queue for the whole app: at most two captures run at a time (ADR 0002).
// Created on first use so environments without IndexedDB can still import this module.
let thumbnailCache: ThumbnailCache | null = null;
const thumbnailQueue = new ThumbnailTaskQueue();
const mediaUrl = transport.mediaUrl.bind(transport);

type Props = {
	course: CatalogNode;
	profile: string;
	className?: string;
	eager?: boolean;
	/** Lessons finished and total lessons, drawn as the bar under the generated title. */
	done?: number;
	total?: number;
	/** Small thumbnails draw the first letter instead of the title. */
	compact?: boolean;
	/** Draw the lessons as a map instead of the title, where the course name is already the heading. */
	map?: boolean;
};

/**
 * Course cover, by precedence (ADR 0002): the uploaded cover, then an automatic video
 * thumbnail captured once the cover is visible, then a cover drawn from the title and the
 * course's lessons. Decorative: the card or heading next to it names the course. When the title
 * is drawn on the cover the element carries `data-titled`, so the heading beside it can be
 * visually hidden instead of repeating the name.
 */
export function CourseCover({ course, profile, className, eager = false, done = 0, total = 0, compact = false, map = false }: Props) {
	const element = useRef<HTMLSpanElement>(null);
	const [thumbnailUrl, setThumbnailUrl] = useState<string | null>(null);
	const manualCoverUrl = transport.coverUrl(courseIdOf(course), course.coverVersion ?? null);
	const source = selectCourseCoverSource({ manualCoverUrl, thumbnailUrl });

	useEffect(() => {
		const target = element.current;
		if (!target || manualCoverUrl) return;
		let active = true;
		let objectUrl: string | null = null;
		const visibility = observeThumbnailVisibility(target, async () => {
			thumbnailCache ??= new ThumbnailCache(new IndexedDbThumbnailCacheAdapter());
			const blob = await loadCourseThumbnail({ course, profile, cache: thumbnailCache, queue: thumbnailQueue, mediaUrl, manualCoverUrl });
			if (!blob || !active) return;
			objectUrl = URL.createObjectURL(blob);
			setThumbnailUrl(objectUrl);
		});
		return () => {
			active = false;
			visibility.destroy();
			if (objectUrl) URL.revokeObjectURL(objectUrl);
			setThumbnailUrl(null);
		};
	}, [course, profile, manualCoverUrl]);

	const image = source === 'manual' ? manualCoverUrl : source === 'thumbnail' ? thumbnailUrl : null;
	return (
		<span ref={element} aria-hidden="true" className={cn('relative block aspect-[16/10] overflow-hidden rounded-[1.25rem] bg-muted', className)}>
			{image ? (
				<img src={image} alt="" width={600} height={400} loading={eager ? 'eager' : 'lazy'} decoding="async" className="size-full object-cover" />
			) : (
				<CoverArt title={course.title} done={done} total={total} compact={compact} map={map} />
			)}
		</span>
	);
}
