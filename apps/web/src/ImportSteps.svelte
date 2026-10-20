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
	.flow-steps { list-style: none; margin: 16px 0; padding: 0; display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 8px; counter-reset: step; }
	.flow-step { display: grid; gap: 3px; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); font-size: 13px; color: var(--muted); }
	.flow-step b { color: var(--ink); font-size: 13px; }
	.flow-step.current { border-color: var(--accent); background: var(--accent-soft); }
	.flow-step.current b { color: var(--accent); }
	.flow-step.done b { color: var(--accent); }
	.small-screen-note { display: none; margin: 0; color: var(--muted); font-size: 13px; }
	@media (max-width: 700px) {
		.flow-steps { display: none; }
		.small-screen-note { display: block; }
	}
</style>
