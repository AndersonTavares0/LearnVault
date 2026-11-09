import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { BrandMark } from '@/components/brand';
import { courseCover, selectCourseCoverSource } from '@/lib/cover';
import { IndexedDbThumbnailCacheAdapter, loadCourseThumbnail, observeThumbnailVisibility, ThumbnailCache, ThumbnailTaskQueue } from '@/lib/thumbnails';
import { courseIdOf, transport, type CatalogNode } from '@/lib/transport';
import { cn } from '@/lib/utils';

// One cache and one queue for the whole app: at most two captures run at a time (ADR 0002).
// Created on first use so environments without IndexedDB can still import this module.
let thumbnailCache: ThumbnailCache | null = null;
const thumbnailQueue = new ThumbnailTaskQueue();
const mediaUrl = transport.mediaUrl.bind(transport);

/**
 * Course cover, by precedence (ADR 0002): the uploaded cover, then an automatic video
 * thumbnail captured once the cover is visible, then a cover generated from the title.
 * Decorative: the card or heading next to it names the course.
 */
export function CourseCover({ course, profile, className, eager = false }: { course: CatalogNode; profile: string; className?: string; eager?: boolean }) {
	const element = useRef<HTMLSpanElement>(null);
	const [thumbnailUrl, setThumbnailUrl] = useState<string | null>(null);
	const manualCoverUrl = transport.coverUrl(courseIdOf(course), course.coverVersion ?? null);
	const generated = useMemo(() => courseCover(course.title), [course.title]);
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
		<span
			ref={element}
			aria-hidden="true"
			className={cn('relative block aspect-[3/2] overflow-hidden rounded-lg bg-muted', className)}
			// Custom properties are set through the CSSOM, which the server CSP allows.
			style={{ '--cover-from': generated.from, '--cover-to': generated.to, '--cover-angle': `${generated.angle}deg` } as CSSProperties}
		>
			{image ? (
				<img src={image} alt="" width={600} height={400} loading={eager ? 'eager' : 'lazy'} decoding="async" className="size-full object-cover" />
			) : (
				<span className="flex size-full items-end justify-between bg-[linear-gradient(var(--cover-angle),var(--cover-from),var(--cover-to))] p-3 text-white/90">
					<span className="font-mono text-xs tracking-wider">{generated.code}</span>
					<BrandMark className="size-6 opacity-70" />
				</span>
			)}
		</span>
	);
}
