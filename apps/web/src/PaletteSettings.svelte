<script lang="ts">
	import {
		DEFAULT_PALETTE_ID,
		PALETTE_COLOR_FIELDS,
		PALETTE_PRESETS,
		validatePalette,
		type Palette,
		type PaletteColors,
		type PalettePreferences
	} from './palettes';

	let { preferences, onChange }: { preferences: PalettePreferences; onChange: (next: PalettePreferences) => boolean } = $props();

	const FIELD_LABELS: Record<keyof PaletteColors, string> = {
		background: 'Fundo',
		surface: 'Superfície',
		surfaceRaised: 'Superfície elevada',
		input: 'Campo',
		text: 'Texto',
		muted: 'Texto auxiliar',
		border: 'Borda',
		borderStrong: 'Borda forte',
		accent: 'Destaque',
		onAccent: 'Texto sobre destaque',
		navigation: 'Navegação',
		onNavigation: 'Texto da navegação',
		danger: 'Erro',
		warning: 'Aviso'
	};

	type Variant = 'light' | 'dark';
	const VARIANT_LABELS: Record<Variant, string> = { light: 'Claro', dark: 'Escuro' };

	let draft = $state<Palette | null>(null);
	let base = $state<Palette | null>(null);
	let variant = $state<Variant>('light');
	let editingId = $state<string | null>(null);
	let confirmingDelete = $state<string | null>(null);
	let message = $state('');

	let palettes = $derived([...PALETTE_PRESETS, ...preferences.customPalettes]);
	let selectedId = $derived(preferences.selectedPaletteId);
	let errors = $derived(draft ? validatePalette(draft) : []);
	let nameError = $derived(draft ? errors.find((error) => error.field === 'name')?.message ?? '' : '');
	let variantErrors = $derived(
		draft
			? errors.filter((error) => error.field.startsWith(`${variant}.`)).map((error) => ({
					label: FIELD_LABELS[error.field.split('.')[1] as keyof PaletteColors] ?? error.field,
					message: error.message
				}))
			: []
	);
	let selected = $derived(palettes.find((palette) => palette.id === selectedId) ?? PALETTE_PRESETS[0]);
	let draftColors = $derived(draft ? draft[variant] : null);
	let previewVars = $derived(
		draftColors
			? {
					'--preview-bg': draftColors.background,
					'--preview-surface': draftColors.surface,
					'--preview-input': draftColors.input,
					'--preview-ink': draftColors.text,
					'--preview-muted': draftColors.muted,
					'--preview-line': draftColors.border,
					'--preview-accent': draftColors.accent,
					'--preview-on-accent': draftColors.onAccent,
					'--preview-nav': draftColors.navigation,
					'--preview-on-nav': draftColors.onNavigation,
					'--preview-danger': draftColors.danger
				}
			: null
	);
	let previewStyle = $derived(previewVars ? Object.entries(previewVars).map(([name, value]) => `${name}: ${value}`).join(';') : '');

	function nameIsAvailable(name: string, ignoreId: string | null): boolean {
		const normalized = name.trim().toLowerCase();
		return !palettes.some((palette) => palette.id !== ignoreId && palette.name.toLowerCase() === normalized);
	}

	function startCreate(): void {
		const origin = selected;
		const id = `custom-${crypto.randomUUID()}`;
		const next: Palette = {
			id,
			name: origin.name === 'Pedra, areia e oliva' ? 'Meu estudo' : `${origin.name} (cópia)`,
			light: $state.snapshot(origin.light),
			dark: $state.snapshot(origin.dark)
		};
		draft = next;
		base = $state.snapshot(next);
		editingId = null;
		variant = 'light';
		confirmingDelete = null;
		message = '';
	}

	function startEdit(palette: Palette): void {
		draft = $state.snapshot(palette);
		base = $state.snapshot(palette);
		editingId = palette.id;
		variant = 'light';
		confirmingDelete = null;
		message = '';
	}

	function restoreVariant(): void {
		if (!draft || !base) return;
		const restored: Palette = { ...draft, [variant]: $state.snapshot(base[variant]) };
		draft = restored;
		message = `Variante ${VARIANT_LABELS[variant].toLowerCase()} restaurada.`;
	}

	function cancelEdit(): void {
		draft = null;
		base = null;
		editingId = null;
		confirmingDelete = null;
		message = '';
	}

	function save(): void {
		if (!draft) return;
		if (!nameIsAvailable(draft.name, editingId)) {
			message = 'Já existe uma paleta com este nome. Escolha outro.';
			return;
		}
		const remaining = validatePalette(draft);
		if (remaining.length) {
			message = 'Corrija os problemas indicados antes de salvar.';
			return;
		}
		const customPalettes = preferences.customPalettes.filter((palette) => palette.id !== editingId);
		const saved: Palette = { id: draft.id, name: draft.name.trim(), light: $state.snapshot(draft.light), dark: $state.snapshot(draft.dark) };
		const next: PalettePreferences = { version: 1, selectedPaletteId: saved.id, customPalettes: [...customPalettes, saved] };
		const persisted = onChange(next);
		draft = null;
		base = null;
		editingId = null;
		message = persisted
			? 'Paleta salva e aplicada. Salva neste navegador/dispositivo; não sincronizada com a conta.'
			: 'Paleta aplicada nesta sessão; não foi possível salvar neste navegador.';
	}

	function select(id: string): void {
		if (draft) return;
		onChange({ ...preferences, selectedPaletteId: id });
		confirmingDelete = null;
		message = '';
	}

	function requestDelete(id: string): void {
		confirmingDelete = confirmingDelete === id ? null : id;
		message = '';
	}

	function deleteCustom(id: string): void {
		const customPalettes = preferences.customPalettes.filter((palette) => palette.id !== id);
		const selectedPaletteId = selectedId === id ? DEFAULT_PALETTE_ID : selectedId;
		onChange({ ...preferences, selectedPaletteId, customPalettes });
		confirmingDelete = null;
		message = selectedId === id ? 'Paleta excluída. Pedra, areia e oliva volta a ser ativa.' : 'Paleta excluída.';
	}
</script>

<div class="palette-settings">
	<ul class="palette-list" aria-label="Paletas disponíveis">
		{#each palettes as palette (palette.id)}
			{@const isDraft = draft?.id === palette.id}
			<li class="palette-option" class:selected={palette.id === selectedId}>
				<button
					type="button"
					class="palette-choice"
					aria-pressed={palette.id === selectedId}
					onclick={() => select(palette.id)}
					disabled={Boolean(draft)}
				>
					<span class="swatches" aria-hidden="true">
						<span class="swatch" style:background={palette.light.background}><i style:background={palette.light.accent}></i><i style:background={palette.light.text}></i></span>
						<span class="swatch" style:background={palette.dark.background}><i style:background={palette.dark.accent}></i><i style:background={palette.dark.text}></i></span>
					</span>
					<span class="palette-name">{palette.name}</span>
					<span class="small">{palette.id === selectedId ? 'Ativa' : 'Claro e escuro'}</span>
				</button>
				<div class="palette-tools">
					{#if PALETTE_PRESETS.some((preset) => preset.id === palette.id)}
						<button class="text-button" type="button" onclick={() => startCreate()} disabled={Boolean(draft)}>Duplicar e editar</button>
					{:else}
						<button class="text-button" type="button" onclick={() => startEdit(palette)} disabled={Boolean(draft) && !isDraft}>Editar</button>
						{#if confirmingDelete === palette.id}
							<span class="confirm">
								<span>Excluir esta paleta?</span>
								<button class="text-button danger" type="button" onclick={() => deleteCustom(palette.id)}>Excluir</button>
								<button class="text-button" type="button" onclick={() => (confirmingDelete = null)}>Cancelar</button>
							</span>
						{:else}
							<button class="text-button danger" type="button" onclick={() => requestDelete(palette.id)} disabled={Boolean(draft)}>Excluir</button>
						{/if}
					{/if}
				</div>
			</li>
		{/each}
	</ul>

	{#if !draft}
		<button class="btn primary" type="button" onclick={startCreate}>Criar tema</button>
	{/if}

	{#if draft}
		<section class="editor" aria-label="Editor de paleta">
			<div class="editor-head">
				<h4>{editingId ? 'Editar paleta' : 'Nova paleta'}</h4>
				<div class="variant-toggle" role="group" aria-label="Variante em edição">
					<button class="btn" class:active={variant === 'light'} type="button" aria-pressed={variant === 'light'} onclick={() => (variant = 'light')}>Claro</button>
					<button class="btn" class:active={variant === 'dark'} type="button" aria-pressed={variant === 'dark'} onclick={() => (variant = 'dark')}>Escuro</button>
				</div>
			</div>

			<label class="form-field" for="palette-name">Nome da paleta
				<input id="palette-name" name="palette-name" class="field" bind:value={draft.name} maxlength={40} required aria-invalid={nameError ? 'true' : undefined} aria-describedby={nameError ? 'palette-name-error' : undefined} />
			</label>
			{#if nameError}<p id="palette-name-error" class="error" role="alert">{nameError}</p>{/if}

			{#if draftColors}
			<div class="color-grid">
				{#each PALETTE_COLOR_FIELDS as field (field)}
					<label class="form-field" for={`palette-${variant}-${field}`}>
						{FIELD_LABELS[field]}
						<span class="color-input">
							<input type="color" aria-label={`${FIELD_LABELS[field]} (seletor)`} value={draftColors[field]} oninput={(event) => (draft = { ...draft!, [variant]: { ...draft![variant], [field]: event.currentTarget.value.toUpperCase() } })} />
							<input id={`palette-${variant}-${field}`} name={`palette-${variant}-${field}`} class="field hex" bind:value={draft[variant][field]} aria-label={`${FIELD_LABELS[field]} (hexadecimal)`} spellcheck={false} />
						</span>
					</label>
				{/each}
			</div>
			{#if variantErrors.length}
				<ul class="error-list" role="alert" aria-label="Problemas de contraste nesta variante">
					{#each variantErrors as error (error.label + error.message)}
						<li><strong>{error.label}:</strong> {error.message}</li>
					{/each}
				</ul>
			{/if}
			{/if}

			<div class="preview" style={previewStyle}>
				<div class="preview-header"><strong>Prévia</strong><span>Aula em andamento</span></div>
				<div class="preview-card">
					<p>Assim ficam o texto, os campos e os botões nesta variante.</p>
					<input class="preview-field" value="Campo de exemplo" readonly tabindex="-1" />
					<div class="preview-row">
						<button class="preview-button" type="button" tabindex="-1">Ação</button>
						<span class="preview-error">Erro de exemplo</span>
					</div>
				</div>
			</div>

			<div class="editor-actions">
				<button class="btn primary" type="button" onclick={save}>Salvar e aplicar</button>
				<button class="btn" type="button" onclick={restoreVariant}>Restaurar esta variante</button>
				<button class="btn" type="button" onclick={cancelEdit}>Cancelar</button>
			</div>
		</section>
	{/if}

	{#if message}<p class="inline-status" role="status">{message}</p>{/if}
	<p class="small persistence-note">Salvo neste navegador/dispositivo; não sincronizado com a conta.</p>
</div>

<style>
	.palette-settings { display: grid; gap: 16px; }
	.palette-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
	.palette-option { display: flex; align-items: stretch; gap: 8px; flex-wrap: wrap; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); padding: 8px; }
	.palette-option.selected { border-color: var(--accent); }
	.palette-choice { display: grid; grid-template-columns: auto auto; align-items: center; gap: 4px 12px; border: 0; background: transparent; color: var(--ink); text-align: left; padding: 4px; min-height: 44px; }
	.palette-choice:focus-visible { outline: 3px solid var(--accent); outline-offset: 2px; }
	.palette-choice .palette-name { font-weight: 650; font-size: 14px; }
	.palette-choice .small { grid-column: 2; color: var(--muted); font-size: 13px; }
	.palette-option.selected .palette-choice .small { color: var(--accent); font-weight: 700; }
	.swatches { display: grid; grid-template-columns: 44px 44px; gap: 4px; }
	.swatch { display: inline-flex; gap: 3px; align-items: center; justify-content: center; width: 44px; height: 28px; border-radius: 6px; border: 1px solid var(--line); }
	.swatch i { width: 10px; height: 10px; border-radius: 3px; }
	.palette-tools { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-left: auto; }
	.confirm { display: inline-flex; align-items: center; gap: 8px; color: var(--muted); font-size: 13px; }
	.editor { display: grid; gap: 14px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface-raised); }
	.editor-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
	.editor h4 { margin: 0; font-size: 16px; }
	.variant-toggle { display: inline-flex; gap: 8px; }
	.variant-toggle .btn.active { border-color: var(--accent); background: var(--accent-soft); color: var(--accent); font-weight: 700; }
	.color-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px 16px; }
	.color-input { display: flex; gap: 8px; margin-top: 5px; }
	.color-input input[type='color'] { width: 44px; height: 44px; padding: 4px; border: 1px solid var(--line-strong); border-radius: var(--radius-sm); background: var(--input); }
	.color-input .hex { flex: 1; min-width: 0; }
	.error-list { margin: 0; padding: 12px 14px; list-style: none; border: 1px solid var(--line); border-left: 3px solid var(--danger); border-radius: var(--radius-sm); background: var(--danger-soft); color: var(--danger); font-size: 13px; display: grid; gap: 6px; }
	.preview { display: grid; gap: 10px; padding: 14px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--preview-bg); color: var(--preview-ink); }
	.preview-header { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-radius: var(--radius-sm); background: var(--preview-nav); color: var(--preview-on-nav); }
	.preview-card { display: grid; gap: 10px; padding: 12px; border: 1px solid var(--preview-line); border-radius: var(--radius-sm); background: var(--preview-surface); }
	.preview-card p { margin: 0; color: var(--preview-muted); font-size: 13px; }
	.preview-field { height: 44px; padding: 8px 12px; border: 1px solid var(--preview-line); border-radius: 8px; background: var(--preview-input); color: var(--preview-ink); font: inherit; font-size: 16px; }
	.preview-row { display: flex; align-items: center; gap: 12px; }
	.preview-button { min-height: 44px; padding: 8px 16px; border: 0; border-radius: 8px; background: var(--preview-accent); color: var(--preview-on-accent); font-weight: 650; }
	.preview-error { color: var(--preview-danger); font-size: 13px; font-weight: 650; }
	.editor-actions { display: flex; gap: 8px; flex-wrap: wrap; }
	.persistence-note { margin: 0; }
	@media (max-width: 700px) {
		.color-grid { grid-template-columns: minmax(0, 1fr); }
		.editor-actions .btn { flex: 1; }
	}
</style>
