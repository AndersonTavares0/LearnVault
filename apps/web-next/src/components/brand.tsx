/** LV monogram (docs/design/branding/learnvault-mark.svg), drawn in the current text color. */
export function BrandMark({ className }: { className?: string }) {
	return (
		<svg className={className} viewBox="0 0 128 128" aria-hidden="true" focusable="false">
			<path d="M18 25H35L49 83H68L61 101H37C31 101 27 97 25 91L18 25Z" fill="currentColor" />
			<path d="M43 25H61L79 75L98 25H116L88 94C86 99 82 102 76 102H72C66 102 62 99 60 94L43 47H61L74 83L91 40L79 75L61 25H43Z" fill="currentColor" />
		</svg>
	);
}
