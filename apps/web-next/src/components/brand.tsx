/**
 * LV monogram (docs/design/branding/learnvault-mark.svg): the project's initials as one shape,
 * the foot of the L running under the V. The tile takes the text color (use `text-primary`), so
 * it follows the palette; the letters are the primary foreground, the V one step softer.
 */
export function BrandMark({ className }: { className?: string }) {
	return (
		<svg className={className} viewBox="0 0 128 128" aria-hidden="true" focusable="false">
			<rect width="128" height="128" rx="30" fill="currentColor" />
			<path d="M20 30H38V84H62V100H20Z" fill="var(--primary-foreground)" />
			<path d="M56 30H75L87 76L99 30H118L97 100H77Z" fill="var(--primary-foreground)" fillOpacity="0.75" />
		</svg>
	);
}
