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

/** A low-resolution check avoids displaying nearly solid frames as course artwork. */
function hasImageDetail(image: HTMLImageElement): boolean {
	try {
		const canvas = document.createElement('canvas');
		// 32×32 keeps a slide's title line visible; 8×8 averaged it away and hid real lecture frames.
		canvas.width = canvas.height = 32;
		const context = canvas.getContext('2d');
		if (!context) return true;
		context.drawImage(image, 0, 0, 32, 32);
		const { data } = context.getImageData(0, 0, 32, 32);
		for (let i = 4; i < data.length; i += 4) {
			if (Math.abs(data[i] - data[0]) > 16 || Math.abs(data[i + 1] - data[1]) > 16 || Math.abs(data[i + 2] - data[2]) > 16 || Math.abs(data[i + 3] - data[3]) > 16) return true;
		}
		return false;
	} catch {
		// Some transports prevent reading pixels. Keep an image that cannot be inspected.
		return true;
	}
}

/** Decorative course art: a detailed source image or a generated cover. */
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
		<span ref={element} aria-hidden="true" className={cn('relative block aspect-[4/3] overflow-hidden rounded-[var(--radius)] bg-muted ring-1 ring-inset ring-foreground/10', className)}>
			<CoverArt title={course.title} courseId={courseIdOf(course)} done={done} total={total} compact={compact} map={map} />
			{map && !compact && <span className="sr-only">{course.title}</span>}
			{image && <img key={image} src={image} alt="" width={600} height={400} loading={eager ? 'eager' : 'lazy'} decoding="async" className="absolute inset-0 size-full object-cover opacity-0 data-[ready]:opacity-100" onLoad={({ currentTarget }) => {
				if (hasImageDetail(currentTarget)) currentTarget.dataset.ready = '';
			}} />}
		</span>
	);
}
