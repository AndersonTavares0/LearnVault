import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

/**
 * The Academy Portico (docs/design/branding/learnvault-mark.svg): a Greek school front with an arched
 * vault door. The stone follows `currentColor` (pass `text-foreground`, so it is cream on the dark page and
 * ink on the light one); the door and the pediment dot use the reward colour, the single accent.
 * `compact` is the reduced cut (two columns, wider door) for sizes of 24 px and below.
 */
export function BrandMark({ className, compact = false }: { className?: string; compact?: boolean }) {
	return (
		<svg className={className} viewBox="0 0 128 128" aria-hidden="true" focusable="false">
			{compact ? (
				<>
					<path d="M64 14L116 44H12Z" fill="currentColor" />
					<path d="M16 50H112V58H16Z" fill="currentColor" />
					<path d="M22 64H40V104H22ZM88 64H106V104H88Z" fill="currentColor" />
					<path d="M50 104V86A14 14 0 0 1 78 86V104Z" fill="var(--reward-fill)" />
					<path d="M10 104H118V114H10Z" fill="currentColor" />
				</>
			) : (
				<>
					<path d="M64 12L118 42H10Z" fill="currentColor" />
					<circle cx="64" cy="33" r="4.5" fill="var(--reward-fill)" />
					<path d="M14 46H114V53H14Z" fill="currentColor" />
					<path d="M20 58H32V104H20ZM40 58H52V104H40ZM76 58H88V104H76ZM96 58H108V104H96Z" fill="currentColor" />
					<path d="M55 104V84A9 9 0 0 1 73 84V104Z" fill="var(--reward-fill)" />
					<path d="M12 104H116V110H12ZM8 110H120V117H8Z" fill="currentColor" />
				</>
			)}
		</svg>
	);
}

type MottoName = 'sapereAude' | 'nullaDies' | 'adFontes' | 'archeHemisy';

/**
 * The mottos from docs/design/branding. The Latin or Greek line is the quotation; the learner's
 * language is the readable one. Greek is set in Literata because Playfair Display has no Greek glyphs.
 */
const MOTTOS: Record<MottoName, { quote: string; lang: 'la' | 'grc'; translation: () => string }> = {
	sapereAude: { quote: 'Sapere aude', lang: 'la', translation: () => tr('Dare to know', 'Ouse saber') },
	nullaDies: { quote: 'Nulla dies sine linea', lang: 'la', translation: () => tr('Not a day without a line', 'Nenhum dia sem uma linha') },
	adFontes: { quote: 'Ad fontes', lang: 'la', translation: () => tr('Back to the sources', 'De volta às fontes') },
	archeHemisy: { quote: 'ἀρχὴ ἥμισυ παντός', lang: 'grc', translation: () => tr('The beginning is half of the whole', 'O começo é metade do todo') }
};

export function Motto({ name, className }: { name: MottoName; className?: string }) {
	const motto = MOTTOS[name];
	return (
		<p className={cn('text-sm text-muted-foreground', className)}>
			<span lang={motto.lang} className={cn('italic', motto.lang === 'grc' ? 'font-reading' : 'font-heading')}>{motto.quote}</span>
			<span aria-hidden="true"> · </span>
			<span>{motto.translation()}</span>
		</p>
	);
}
