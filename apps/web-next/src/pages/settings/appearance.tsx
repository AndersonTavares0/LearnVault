import { Check } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { Panel } from '@/components/panel';
import { ACCENT_PRESETS, appearanceStore, darkAccent, DENSITIES, FONTS, RADII, readableAccent, resetAppearance, resolveTheme, REWARD_COLORS, SIDEBARS, TEXT_SIZES, THEMES, updateAppearance, type Appearance } from '@/lib/appearance';
import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import { safeStorage } from '@/app/preferences';

const accentNames = (): Record<(typeof ACCENT_PRESETS)[number]['id'], string> => ({ brown: tr('Brown', 'Marrom'), amber: tr('Amber', 'Âmbar'), forest: tr('Forest', 'Floresta'), terracotta: tr('Terracotta', 'Terracota'), navy: tr('Navy', 'Azul-marinho'), plum: tr('Plum', 'Ameixa'), teal: tr('Teal', 'Verde-azulado'), slate: tr('Slate', 'Ardósia') });

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
		<div className="space-y-6">
			<div className="grid gap-6 xl:grid-cols-2">
			<Panel className="space-y-6 p-5 sm:p-6">
				<h2 className="font-heading text-xl font-semibold">{tr('Colour', 'Cor')}</h2>
			<Choice legend={tr('Theme', 'Tema')} options={THEMES} value={value.theme} label={(option) => ({ light: tr('Light', 'Claro'), dark: tr('Dark', 'Escuro'), system: tr('Follow the system', 'Seguir o sistema') })[option]} onChange={(theme) => change({ theme })} />
			<fieldset className="space-y-3">
				<legend className="text-sm font-semibold">{tr('Accent color', 'Cor de destaque')}</legend>
				<div className="flex flex-wrap gap-2">
					{ACCENT_PRESETS.map((preset) => {
						const selected = value.accent === preset.hex;
						return (
							<button key={preset.id} type="button" aria-pressed={selected} onClick={() => { change({ accent: preset.hex }); setCustom(preset.hex); }} className={cn('flex min-h-11 items-center gap-2 rounded-lg border px-3 text-sm font-medium hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring focus-visible:outline-none', selected && 'border-primary bg-brand-tint')}>
								<span aria-hidden="true" className="grid size-5 place-items-center rounded-full text-brand-foreground" style={{ backgroundColor: dark ? preset.dark : preset.hex }}>{selected && <Check className="size-3.5" />}</span>
								{accentNames()[preset.id]}
							</button>
						);
					})}
				</div>
				<div className="flex flex-wrap items-end gap-3">
					<div className="space-y-1.5">
						<Label htmlFor="accent-custom" className="text-sm">{tr('Your own color', 'Sua própria cor')}</Label>
						<div className="flex items-center gap-2">
							<input aria-label={tr('Pick a color', 'Escolher uma cor')} type="color" value={custom} onChange={(event) => setCustom(event.target.value.toUpperCase())} className="size-11 cursor-pointer rounded-lg border border-input bg-background p-1" />
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
				<h2 className="font-heading text-xl font-semibold">{tr('Shape and text', 'Forma e texto')}</h2>
			<Choice legend={tr('Corners', 'Cantos')} options={RADII} value={value.radius} label={(option) => ({ square: tr('Square', 'Retos'), soft: tr('Soft', 'Suaves'), round: tr('Round', 'Arredondados') })[option]} onChange={(radius) => change({ radius })} />
			<Choice legend={tr('Density', 'Densidade')} options={DENSITIES} value={value.density} label={(option) => (option === 'comfortable' ? tr('Comfortable', 'Confortável') : tr('Compact', 'Compacta'))} onChange={(density) => change({ density })} />
			<Choice legend={tr('Text size', 'Tamanho do texto')} options={TEXT_SIZES} value={value.textSize} label={(option) => ({ small: tr('Small', 'Pequeno'), medium: tr('Medium', 'Médio'), large: tr('Large', 'Grande') })[option]} onChange={(textSize) => change({ textSize })} />
			<Choice legend={tr('Font', 'Fonte')} options={FONTS} value={value.font} label={(option) => ({ sans: tr('DM Sans with serif titles', 'DM Sans com títulos serifados'), system: tr('System font', 'Fonte do sistema'), serif: tr('Reading serif', 'Serifada de leitura') })[option]} onChange={(font) => change({ font })} />
			</Panel>
			<Panel className="space-y-6 p-5 sm:p-6 xl:col-span-2">
				<h2 className="font-heading text-xl font-semibold">{tr('My learning layout', 'Layout do Meu aprendizado')}</h2>
			<Choice legend={tr('Course content in a lesson', 'Conteúdo do curso na aula')} options={SIDEBARS} value={value.sidebar} label={(option) => (option === 'right' ? tr('On the right', 'À direita') : tr('On the left', 'À esquerda'))} onChange={(sidebar) => change({ sidebar })} />
			<fieldset className="space-y-2">
				<legend className="text-sm font-semibold">{tr('What My learning shows', 'O que Meu aprendizado mostra')}</legend>
				<Toggle label={tr('Continue learning row', 'Linha “Continuar estudando”')} checked={value.showContinue} onChange={(showContinue) => change({ showContinue })} />
				<Toggle label={tr('Weekly goal', 'Meta semanal')} checked={value.showGoal} onChange={(showGoal) => change({ showGoal })} />
				<Toggle label={tr('Streak', 'Sequência')} checked={value.showStreak} onChange={(showStreak) => change({ showStreak })} />
				<Toggle label={tr('Level and XP', 'Nível e XP')} checked={value.showXp} onChange={(showXp) => change({ showXp })} />
				<Toggle label={tr('Badges', 'Emblemas')} checked={value.showBadges} onChange={(showBadges) => change({ showBadges })} />
			</fieldset>
			</Panel>
			</div>
			<Button variant="outline" onClick={() => { const next = resetAppearance(safeStorage()); setCustom(next.accent); }}>{tr('Reset to the default look', 'Voltar ao visual padrão')}</Button>
		</div>
	);
}

function Choice<T extends string>({ legend, options, value, label, onChange }: { legend: string; options: readonly T[]; value: T; label: (value: T) => ReactNode; onChange: (value: T) => void }) {
	return (
		<fieldset className="space-y-2">
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

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
	const id = useId();
	return (
		<label htmlFor={id} className="flex min-h-11 cursor-pointer items-center gap-3 text-sm">
			<input id={id} type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} className="size-5 accent-[var(--brand)]" />
			{label}
		</label>
	);
}
