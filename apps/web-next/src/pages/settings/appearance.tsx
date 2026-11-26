import { Check } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { SwitchCard } from './switch-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { Panel } from '@/components/panel';
import { ACCENT_PRESETS, appearanceStore, CARD_STYLES, darkAccent, DENSITIES, FONTS, HOME_LAYOUTS, PAGE_WIDTHS, RADII, readableAccent, resetAppearance, resolveTheme, REWARD_COLORS, SIDEBARS, SURFACE_SWATCHES, SURFACES, TEXT_SIZES, THEMES, updateAppearance, type Appearance } from '@/lib/appearance';
import { tr } from '@/lib/i18n';
import { persistLibraryView } from '@/lib/library-view';
import { cn } from '@/lib/utils';
import { safeStorage } from '@/app/preferences';

const accentNames = (): Record<(typeof ACCENT_PRESETS)[number]['id'], string> => ({ ink: tr('Ink', 'Tinta'), brown: tr('Brown', 'Marrom'), amber: tr('Amber', 'Âmbar'), forest: tr('Forest', 'Floresta'), terracotta: tr('Terracotta', 'Terracota'), navy: tr('Navy', 'Azul-marinho'), plum: tr('Plum', 'Ameixa'), teal: tr('Teal', 'Verde-azulado'), slate: tr('Slate', 'Ardósia') });

const surfaceNames = (): Record<(typeof SURFACES)[number], string> => ({
	warm: tr('Warm paper', 'Papel quente'),
	stone: tr('Cool stone', 'Pedra neutra'),
	crisp: tr('Crisp contrast', 'Alto contraste'),
	sepia: tr('Kindle sepia', 'Sépia Kindle'),
	sage: tr('Forest mist', 'Névoa verde'),
	nord: tr('Nord frost', 'Nord ártico'),
	rose: tr('Rosé pine', 'Rosé pine'),
	solarized: tr('Solarized', 'Solarized')
});

/**
 * Customize: the colour, shape, size and layout of the app, and which parts of the habit tools show.
 * Every change applies at once and is kept in this browser only.
 */
export function AppearanceSettings() {
	useLocale();
	const value = useStore(appearanceStore);
	const [custom, setCustom] = useState(value.accent);
	const change = (patch: Partial<Appearance>) => updateAppearance(patch, safeStorage());
	const isPreset = ACCENT_PRESETS.some((preset) => preset.hex === value.accent);
	const dark = resolveTheme(value.theme) === 'dark';
	const applied = dark ? darkAccent(custom) : readableAccent(custom);
	return (
		<div className="space-y-5">
			<div aria-hidden="true" className="study-surface flex flex-wrap items-center justify-between gap-3 bg-card px-4 py-3">
				<div className="flex min-w-0 items-baseline gap-3">
					<span className="shrink-0 text-[11px] font-semibold tracking-[0.12em] uppercase text-muted-foreground">{surfaceNames()[value.surface]}</span>
					<p className="truncate font-heading text-lg font-semibold text-foreground">{tr('The quiet study desk', 'A mesa de estudo tranquila')}</p>
				</div>
				<div className="flex items-center gap-2 text-xs">
					<span className="inline-flex h-7 items-center gap-1.5 rounded-[var(--radius)] border border-[var(--study-rule)] bg-panel px-2.5 font-medium text-foreground">
						<span className="size-2 rounded-full bg-primary" />
						{tr('Highlight', 'Destaque')}
					</span>
					<span className="inline-flex h-7 items-center rounded-[var(--radius)] bg-primary px-2.5 font-semibold text-primary-foreground">{tr('Primary', 'Principal')}</span>
				</div>
			</div>
			<div className="grid gap-6 xl:grid-cols-2">
			<Panel className="space-y-6 p-5 sm:p-6">
				<h2 className="text-lg font-bold">{tr('Color', 'Cor')}</h2>
			<Choice legend={tr('Theme', 'Tema')} options={THEMES} value={value.theme} label={(option) => ({ light: tr('Light', 'Claro'), dark: tr('Dark', 'Escuro'), system: tr('Follow the system', 'Seguir o sistema') })[option]} onChange={(theme) => change({ theme })} />
			<fieldset className="min-w-0 space-y-2.5">
				<legend className="text-sm font-semibold">{tr('Surface palette', 'Paleta de fundo')}</legend>
				<div className="grid grid-cols-2 gap-2">
					{SURFACES.map((surface) => {
						const selected = value.surface === surface;
						const swatch = SURFACE_SWATCHES[surface];
						return (
							<button
								key={surface}
								type="button"
								aria-pressed={selected}
								onClick={() => change({ surface })}
								className={cn(
									'flex min-h-11 items-center gap-2 rounded-[var(--radius)] border border-[var(--study-rule)] bg-card px-3 text-left text-sm font-medium transition-colors hover:border-[var(--border-strong)] hover:bg-brand-tint/60 focus-visible:ring-3 focus-visible:ring-ring focus-visible:outline-none',
									selected && 'border-primary bg-brand-soft font-semibold text-brand-ink ring-1 ring-primary/20'
								)}
							>
								<span
									aria-hidden="true"
									className="size-4 shrink-0 rounded-[var(--radius-sm)] border"
									style={{
										backgroundColor: dark ? swatch.dark : swatch.light,
										borderColor: dark ? swatch.edgeDark : swatch.edgeLight
									}}
								/>
								<span className="min-w-0 flex-1 truncate">{surfaceNames()[surface]}</span>
								{selected && <Check className="size-3.5 shrink-0 text-primary" aria-hidden="true" />}
							</button>
						);
					})}
				</div>
			</fieldset>
			<fieldset className="min-w-0 space-y-3">
				<legend className="text-sm font-semibold">{tr('Accent color', 'Cor de destaque')}</legend>
				<div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
					{ACCENT_PRESETS.map((preset) => {
						const selected = value.accent === preset.hex;
						return (
							<button key={preset.id} type="button" aria-pressed={selected} onClick={() => { change({ accent: preset.hex }); setCustom(preset.hex); }} className={cn('flex min-h-11 items-center gap-2 rounded-[var(--radius)] border border-[var(--study-rule)] bg-card px-3 text-sm font-medium transition-colors hover:border-[var(--border-strong)] hover:bg-brand-tint/60 focus-visible:ring-3 focus-visible:ring-ring focus-visible:outline-none', selected && 'border-primary bg-brand-soft font-semibold text-brand-ink ring-1 ring-primary/20')}>
								<span aria-hidden="true" className="grid size-4 shrink-0 place-items-center rounded-full text-brand-foreground" style={{ backgroundColor: dark ? preset.dark : preset.hex }}>{selected && <Check className="size-3" />}</span>
								<span className="truncate">{accentNames()[preset.id]}</span>
							</button>
						);
					})}
				</div>
				<div className="flex flex-wrap items-end gap-3">
					<div className="space-y-1.5">
						<Label htmlFor="accent-custom" className="text-sm">{tr('Your own color', 'Sua própria cor')}</Label>
						<div className="flex flex-wrap items-center gap-2">
							<input aria-label={tr('Pick a color', 'Escolher uma cor')} type="color" value={custom} onChange={(event) => { const hex = event.target.value.toUpperCase(); setCustom(hex); change({ accent: hex }); }} className="size-11 cursor-pointer rounded-lg border border-input bg-background p-1" />
							<Input id="accent-custom" value={custom} onChange={(event) => setCustom(event.target.value.toUpperCase())} spellCheck={false} maxLength={7} className="w-32 font-mono" />
							<Button variant="outline" disabled={!/^#[0-9A-F]{6}$/.test(custom)} onClick={() => change({ accent: custom })}>{tr('Use this color', 'Usar esta cor')}</Button>
						</div>
					</div>
					{!isPreset && <p className="text-sm text-muted-foreground">{tr(`In use: ${value.accent}.`, `Em uso: ${value.accent}.`)}</p>}
				</div>
				{/^#[0-9A-F]{6}$/.test(custom) && applied !== custom && <p className="text-sm text-muted-foreground">{tr(`This color is too light for white text, so it is used as ${applied}.`, `Esta cor é clara demais para texto branco, então será usada como ${applied}.`)}</p>}
			</fieldset>
			<Choice legend={tr('Rewards (streak, XP, badges)', 'Recompensas (sequência, XP, emblemas)')} options={REWARD_COLORS} value={value.reward} label={(option) => (option === 'amber' ? tr('Amber', 'Âmbar') : tr('Same as accent', 'Igual ao destaque'))} onChange={(reward) => change({ reward })} />
			</Panel>
			<Panel className="space-y-6 p-5 sm:p-6">
				<h2 className="text-lg font-bold">{tr('Shape and text', 'Forma e texto')}</h2>
			<Choice legend={tr('Corners', 'Cantos')} options={RADII} value={value.radius} label={(option) => ({ square: tr('Square', 'Retos'), soft: tr('Soft', 'Suaves'), round: tr('Round', 'Arredondados') })[option]} onChange={(radius) => change({ radius })} />
			<Choice legend={tr('Card style', 'Estilo dos cartões')} options={CARD_STYLES} value={value.cardStyle} label={(option) => ({ bordered: tr('Bordered', 'Com borda'), elevated: tr('Elevated', 'Elevado'), soft: tr('Soft fill', 'Fundo suave') })[option]} onChange={(cardStyle) => change({ cardStyle })} />
			<Choice legend={tr('Density', 'Densidade')} options={DENSITIES} value={value.density} label={(option) => (option === 'comfortable' ? tr('Comfortable', 'Confortável') : tr('Compact', 'Compacta'))} onChange={(density) => change({ density })} />
			<Choice legend={tr('Text size', 'Tamanho do texto')} options={TEXT_SIZES} value={value.textSize} label={(option) => ({ small: tr('Small', 'Pequeno'), medium: tr('Medium', 'Médio'), large: tr('Large', 'Grande') })[option]} onChange={(textSize) => change({ textSize })} />
			<Choice legend={tr('Page width', 'Largura da página')} options={PAGE_WIDTHS} value={value.pageWidth} label={(option) => ({ focused: tr('Focused', 'Focada'), standard: tr('Standard', 'Padrão'), wide: tr('Wide', 'Ampla') })[option]} onChange={(pageWidth) => change({ pageWidth })} />
			<Choice legend={tr('Font', 'Fonte')} options={FONTS} value={value.font} label={(option) => ({ sans: tr('DM Sans with serif titles', 'DM Sans com títulos serifados'), modern: tr('Clean sans-serif', 'Sans-serif limpa'), system: tr('System font', 'Fonte do sistema'), serif: tr('Reading serif', 'Serifada de leitura') })[option]} onChange={(font) => change({ font })} />
						<SwitchCard label={tr('Reduce motion & animations', 'Reduzir animações e movimento')} checked={value.reduceMotion} onChange={(reduceMotion) => change({ reduceMotion })} />
			</Panel>
			<Panel className="space-y-6 p-5 sm:p-6 xl:col-span-2">
				<h2 className="text-lg font-bold">{tr('My learning layout', 'Layout do Meu aprendizado')}</h2>
				<div className="grid gap-6 sm:grid-cols-2">
					<Choice
						legend={tr('Default My learning view', 'Visualização padrão de Meu aprendizado')}
						options={HOME_LAYOUTS}
						value={value.homeLayout}
						label={(option) => (option === 'grid' ? tr('Grid', 'Grade') : tr('List', 'Lista'))}
						onChange={(homeLayout) => {
							change({ homeLayout });
							persistLibraryView(safeStorage(), homeLayout === 'list' ? 'list' : 'covers');
						}}
					/>
					<Choice legend={tr('Course content in a lesson', 'Conteúdo do curso na aula')} options={SIDEBARS} value={value.sidebar} label={(option) => (option === 'right' ? tr('On the right', 'À direita') : tr('On the left', 'À esquerda'))} onChange={(sidebar) => change({ sidebar })} />
				</div>
				<fieldset className="min-w-0 space-y-3">
					<legend className="text-sm font-semibold">{tr('What My learning shows', 'O que Meu aprendizado mostra')}</legend>
					<div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-4">
						<SwitchCard label={tr('Continue learning row', 'Linha “Continuar estudando”')} checked={value.showContinue} onChange={(showContinue) => change({ showContinue })} />
						<SwitchCard label={tr('Weekly goal', 'Meta semanal')} checked={value.showGoal} onChange={(showGoal) => change({ showGoal })} />
						<SwitchCard label={tr('Streak', 'Sequência')} checked={value.showStreak} onChange={(showStreak) => change({ showStreak })} />
						<SwitchCard label={tr('Level and XP', 'Nível e XP')} checked={value.showXp} onChange={(showXp) => change({ showXp })} />
						<SwitchCard label={tr('Badges', 'Emblemas')} checked={value.showBadges} onChange={(showBadges) => change({ showBadges })} />
						<SwitchCard label={tr('Bookmarks', 'Favoritos')} checked={value.showBookmarks} onChange={(showBookmarks) => change({ showBookmarks })} />
						<SwitchCard label={tr('Course progress bars', 'Barras de progresso nos cursos')} checked={value.showProgress} onChange={(showProgress) => change({ showProgress })} />
						<SwitchCard label={tr('Course categories', 'Categorias dos cursos')} checked={value.showCategories} onChange={(showCategories) => change({ showCategories })} />
					</div>
				</fieldset>
			</Panel>
			</div>
			<Button variant="outline" onClick={() => { const next = resetAppearance(safeStorage()); persistLibraryView(safeStorage(), 'covers'); setCustom(next.accent); }}>{tr('Reset to the default look', 'Voltar ao visual padrão')}</Button>
		</div>
	);
}

function Choice<T extends string>({ legend, options, value, label, onChange }: { legend: string; options: readonly T[]; value: T; label: (value: T) => ReactNode; onChange: (value: T) => void }) {
	return (
		<fieldset className="min-w-0 space-y-2">
			<legend className="text-sm font-semibold">{legend}</legend>
			<div className="flex flex-wrap gap-2">
				{options.map((option) => (
					<Button key={option} variant={option === value ? 'selected' : 'outline'} aria-pressed={option === value} onClick={() => onChange(option)}>
						{label(option)}
						{option === value && <Check className="size-3.5 text-primary" aria-hidden="true" />}
					</Button>
				))}
			</div>
		</fieldset>
	);
}
