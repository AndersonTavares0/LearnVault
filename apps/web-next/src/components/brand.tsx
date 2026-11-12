/**
 * LV monogram (docs/design/branding/learnvault-mark.svg): the project's initials on a neutral tile,
 * black with white letters on the light theme and the reverse on the dark one, so it never
 * competes with the learner's accent colour.
 */
export function BrandMark({ className }: { className?: string }) {
	return (
		<svg className={className} viewBox="0 0 128 128" aria-hidden="true" focusable="false">
			<rect width="128" height="128" rx="30" fill="var(--foreground)" />
			<path d="M18 28H40V82H64V102H18Z" fill="var(--background)" />
			<path d="M54 28H78L89 72L100 28H124L100 102H78Z" fill="var(--background)" />
		</svg>
	);
}
