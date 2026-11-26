import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

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
