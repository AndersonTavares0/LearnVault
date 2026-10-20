<script lang="ts">
	let { currentStep }: { currentStep: 1 | 2 | 3 | 4 | 5 } = $props();

	const steps = [
		{ number: 1, title: 'Escolher escopo', description: 'Selecionar uma pasta de curso.' },
		{ number: 2, title: 'Descobrir', description: 'Listar arquivos, pastas e estados de acesso.' },
		{ number: 3, title: 'Revisar sugestões', description: 'Conferir hierarquia e ambiguidades.' },
		{ number: 4, title: 'Confirmar importação', description: 'Registrar o catálogo sem alterar arquivos.' },
		{ number: 5, title: 'Reconciliar rescans', description: 'Atualizar o catálogo sem perder ajustes manuais.' }
	] as const;
</script>

<ol class="flow-steps" aria-label="Etapas da importação">
	{#each steps as step (step.number)}
		<li class="flow-step" class:done={step.number < currentStep} class:current={step.number === currentStep} aria-current={step.number === currentStep ? 'step' : undefined}>
			<b>{step.number} · {step.title}</b>{step.description}
		</li>
	{/each}
</ol>
