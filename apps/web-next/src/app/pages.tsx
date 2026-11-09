import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { LOCALE_LABELS, LOCALES, setLocale, tr } from '@/lib/i18n';
import { PALETTES, readPalette, readThemeMode, THEME_MODES, type Palette, type ThemeMode } from '@/lib/theme';
import { safeStorage, savePalette, saveThemeMode } from './preferences';
import { useState } from 'react';

function Placeholder({ title, issue }: { title: string; issue: number }) {
	useLocale();
	return (
		<section className="space-y-2">
			<h1 className="text-2xl font-semibold">{title}</h1>
			<p className="text-muted-foreground">{tr(`This screen is being rebuilt (#${issue}).`, `Esta tela está sendo refeita (#${issue}).`)}</p>
		</section>
	);
}

export const NotesPage = () => <Placeholder title={tr('Notes', 'Notas')} issue={339} />;

const paletteLabels: Record<Palette, string> = { emerald: 'Stone + Emerald', teal: 'Zinc + Teal', indigo: 'Zinc + Indigo' };
const themeLabels = (): Record<ThemeMode, string> => ({ system: tr('System', 'Sistema'), light: tr('Light', 'Claro'), dark: tr('Dark', 'Escuro') });

/** Appearance and language only; the rest of Settings lands in Phase 5 (#339). */
export function SettingsPage() {
	const locale = useLocale();
	const [theme, setTheme] = useState(() => readThemeMode(safeStorage()));
	const [palette, setPalette] = useState(() => readPalette(safeStorage()));
	return (
		<section className="space-y-8">
			<h1 className="text-2xl font-semibold">{tr('Settings', 'Ajustes')}</h1>
			<Choice legend={tr('Theme', 'Tema')} options={THEME_MODES} value={theme} label={(mode) => themeLabels()[mode]} onChange={(mode) => { saveThemeMode(mode); setTheme(mode); }} />
			<Choice legend={tr('Color palette', 'Paleta de cores')} options={PALETTES} value={palette} label={(value) => paletteLabels[value]} onChange={(value) => { savePalette(value); setPalette(value); }} />
			<Choice legend={tr('Language', 'Idioma')} options={LOCALES} value={locale} label={(value) => LOCALE_LABELS[value]} onChange={(value) => setLocale(value)} />
		</section>
	);
}

function Choice<T extends string>({ legend, options, value, label, onChange }: { legend: string; options: readonly T[]; value: T; label: (value: T) => string; onChange: (value: T) => void }) {
	return (
		<fieldset className="space-y-2">
			<legend className="font-medium">{legend}</legend>
			<div className="flex flex-wrap gap-2">
				{options.map((option) => (
					<Button key={option} variant={option === value ? 'default' : 'outline'} aria-pressed={option === value} onClick={() => onChange(option)}>
						{label(option)}
					</Button>
				))}
			</div>
		</fieldset>
	);
}
