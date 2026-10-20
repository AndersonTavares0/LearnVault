<script lang="ts">
	let { currentStep, phase = '' as string }: { currentStep: 1 | 2 | 3 | 4 | 5; phase?: string } = $props();

	const steps = [
		{ number: 1, title: 'Escolher escopo', description: 'Selecionar uma pasta de curso.' },
		{ number: 2, title: 'Descobrir', description: 'Listar arquivos, pastas e estados de acesso.' },
		{ number: 3, title: 'Revisar sugestões', description: 'Conferir hierarquia e ambiguidades.' },
		{ number: 4, title: 'Confirmar importação', description: 'Registrar o catálogo sem alterar arquivos.' },
		{ number: 5, title: 'Reconciliar rescans', description: 'Atualizar o catálogo sem perder ajustes manuais.' }
	] as const;
	const current = $derived(steps.find((step) => step.number === currentStep) ?? steps[0]);
</script>

<ol class="flow-steps" aria-label="Etapas da importação">
	{#each steps as step (step.number)}
		<li class="flow-step" class:done={step.number < currentStep} class:current={step.number === currentStep} aria-current={step.number === currentStep ? 'step' : undefined}>
			<b>{step.number} · {step.title}</b><span>{step.description}</span>
		</li>
	{/each}
</ol>
<p class="small-screen-note" aria-live="polite">Etapa {currentStep} de 5 — {current.title}</p>

<style>
	.flow-steps {
		list-style: none;
		margin: 0 0 24px;
		padding: 8px 0 0;
		display: flex;
		flex-wrap: wrap;
		gap: 0;
		border-top: 1px solid var(--line);
		counter-reset: step;
	}

	.flow-step {
		display: grid;
		gap: 2px;
		flex: 1 1 160px;
		min-width: 0;
		padding: 12px 16px 12px 0;
		margin-right: 16px;
		border-right: 1px solid var(--line);
		color: var(--muted);
		font-size: 13px;
		line-height: 20px;
	}

	.flow-step:last-child { border-right: 0; margin-right: 0; }

	.flow-step b {
		color: var(--muted);
		font-size: 14px;
		line-height: 20px;
		font-weight: 650;
		font-variant-numeric: tabular-nums;
	}

	.flow-step span { display: none; }

	.flow-step.current b { color: var(--accent-strong); }

	.flow-step.current span { display: block; color: var(--muted); }

	.flow-step.done b { color: var(--ink); }

	.small-screen-note { display: none; margin: 0 0 20px; color: var(--muted); font-size: 14px; line-height: 20px; }

	@media (max-width: 950px) {
		.flow-step { flex-basis: 140px; padding-right: 12px; margin-right: 12px; }
		.flow-step span { display: none; }
	}

	@media (max-width: 700px) {
		.flow-steps { display: none; }
		.small-screen-note { display: block; }
	}
</style>
