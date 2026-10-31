<script lang="ts">
	import { transport, type Flashcard, type ReviewRating } from './transport';

	let {
		mediaId
	}: {
		mediaId: string | null;
	} = $props();

	let cards: Flashcard[] = $state([]);
	let loading = $state(false);
	let error = $state('');
	let viewMode: 'review' | 'manage' = $state('review');
	let reviewIndex = $state(0);
	let revealed = $state(false);
	let reviewing = $state(false);

	let isFormOpen = $state(false);
	let editingCardId: string | null = $state(null);
	let frontInput = $state('');
	let backInput = $state('');
	let saving = $state(false);
	let deleteConfirmId: string | null = $state(null);

	let nowSeconds = $state(Math.floor(Date.now() / 1000));
	const dueCards = $derived(cards.filter((c) => c.nextDue <= nowSeconds));
	const currentCard = $derived(dueCards[reviewIndex] ?? null);

	$effect(() => {
		const interval = setInterval(() => {
			nowSeconds = Math.floor(Date.now() / 1000);
		}, 10_000);
		return () => clearInterval(interval);
	});

	$effect(() => {
		const id = mediaId;
		if (!id) {
			cards = [];
			error = '';
			reviewIndex = 0;
			revealed = false;
			return;
		}

		let active = true;
		loading = true;
		error = '';

		transport
			.listFlashcards(id)
			.then((items) => {
				if (active) {
					cards = items;
					reviewIndex = 0;
					revealed = false;
				}
			})
			.catch((cause) => {
				if (active) {
					error = cause instanceof Error ? cause.message : 'Erro ao carregar cartões.';
				}
			})
			.finally(() => {
				if (active) loading = false;
			});

		return () => {
			active = false;
		};
	});

	function revealAnswer() {
		revealed = true;
	}

	async function submitReview(rating: ReviewRating) {
		if (!currentCard || reviewing) return;
		reviewing = true;
		error = '';

		try {
			const updated = await transport.reviewFlashcard(currentCard.cardId, rating);
			cards = cards.map((c) => (c.cardId === updated.cardId ? updated : c));
			nowSeconds = Math.floor(Date.now() / 1000);
			revealed = false;
			if (reviewIndex >= dueCards.length - 1) {
				reviewIndex = 0;
			}
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Falha ao registrar revisão.';
		} finally {
			reviewing = false;
		}
	}

	function startCreating() {
		editingCardId = null;
		frontInput = '';
		backInput = '';
		isFormOpen = true;
	}

	function startEditing(card: Flashcard) {
		editingCardId = card.cardId;
		frontInput = card.front;
		backInput = card.back;
		isFormOpen = true;
	}

	function cancelForm() {
		isFormOpen = false;
		editingCardId = null;
		frontInput = '';
		backInput = '';
	}

	async function saveCard() {
		if (!mediaId || saving) return;
		const frontTrimmed = frontInput.trim();
		const backTrimmed = backInput.trim();
		if (!frontTrimmed || !backTrimmed) return;

		saving = true;
		error = '';

		try {
			if (editingCardId) {
				const updated = await transport.updateFlashcard(editingCardId, frontTrimmed, backTrimmed);
				cards = cards.map((c) => (c.cardId === updated.cardId ? updated : c));
			} else {
				const created = await transport.createFlashcard(mediaId, frontTrimmed, backTrimmed);
				cards = [...cards, created];
			}
			nowSeconds = Math.floor(Date.now() / 1000);
			cancelForm();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Falha ao salvar cartão.';
		} finally {
			saving = false;
		}
	}

	function requestDelete(cardId: string) {
		deleteConfirmId = cardId;
	}

	function cancelDelete() {
		deleteConfirmId = null;
	}

	async function executeDelete(cardId: string) {
		if (saving) return;
		saving = true;
		try {
			await transport.deleteFlashcard(cardId);
			cards = cards.filter((c) => c.cardId !== cardId);
			nowSeconds = Math.floor(Date.now() / 1000);
			deleteConfirmId = null;
			if (reviewIndex >= dueCards.length) {
				reviewIndex = Math.max(0, dueCards.length - 1);
			}
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Falha ao excluir cartão.';
		} finally {
			saving = false;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (isFormOpen) {
			if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
				event.preventDefault();
				void saveCard();
			} else if (event.key === 'Escape') {
				event.preventDefault();
				cancelForm();
			}
			return;
		}

		const target = event.target as HTMLElement | null;
		if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) {
			return;
		}

		if (event.ctrlKey || event.metaKey || event.altKey) {
			return;
		}

		if (viewMode === 'review' && currentCard && !reviewing) {
			if (!revealed) {
				if (event.key === ' ' || event.key === 'Enter') {
					event.preventDefault();
					revealAnswer();
				}
			} else {
				if (event.key === '1' || event.key.toLowerCase() === 'a') {
					event.preventDefault();
					void submitReview('again');
				} else if (event.key === '2' || event.key.toLowerCase() === 'g') {
					event.preventDefault();
					void submitReview('good');
				}
			}
		}
	}

	function formatNextGoodInterval(card: Flashcard): string {
		if (card.intervalDays === 0) return '+1 dia';
		const next = Math.min(card.intervalDays * 2, 365);
		return `+${next} dias`;
	}

	function formatDueBadge(nextDue: number): string {
		const diff = nextDue - nowSeconds;
		if (diff <= 0) return 'Vencido (revisar)';
		const hours = Math.round(diff / 3600);
		if (hours < 24) return `Em ${Math.max(1, hours)}h`;
		const days = Math.round(diff / 86400);
		return `Em ${days}d`;
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section class="flashcards-section" aria-label="Cartões de memorização e revisão espaçada">
	<div class="flashcards-header">
		<div class="flashcards-title-group">
			<h3 class="flashcards-title">
				<svg class="flashcards-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<rect x="2" y="4" width="20" height="16" rx="2" />
					<line x1="6" y1="8" x2="10" y2="8" />
					<line x1="6" y1="12" x2="18" y2="12" />
					<line x1="6" y1="16" x2="14" y2="16" />
				</svg>
				Cartões de Memorização
			</h3>
			{#if dueCards.length > 0}
				<span class="flashcards-due-badge" title="Cartões prontos para revisão hoje">
					{dueCards.length} para revisar
				</span>
			{/if}
			<span class="flashcards-total-count" title="Total de cartões cadastrados nesta aula">
				{cards.length} no total
			</span>
		</div>

		<div class="flashcards-controls">
			<div class="mode-toggles" role="tablist" aria-label="Visualização de cartões">
				<button
					class="mode-btn {viewMode === 'review' ? 'mode-btn--active' : ''}"
					role="tab"
					aria-selected={viewMode === 'review'}
					type="button"
					onclick={() => {
						viewMode = 'review';
						cancelForm();
					}}
				>
					Revisar
				</button>
				<button
					class="mode-btn {viewMode === 'manage' ? 'mode-btn--active' : ''}"
					role="tab"
					aria-selected={viewMode === 'manage'}
					type="button"
					onclick={() => {
						viewMode = 'manage';
					}}
				>
					Gerenciar
				</button>
			</div>

			{#if mediaId && !isFormOpen}
				<button class="lv-btn lvui-btn lv-btn--subtle" type="button" onclick={startCreating}>
					<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
						<line x1="12" y1="5" x2="12" y2="19" />
						<line x1="5" y1="12" x2="19" y2="12" />
					</svg>
					Novo Cartão
				</button>
			{/if}
		</div>
	</div>

	{#if error}
		<p class="flashcards-error" role="alert">{error}</p>
	{/if}

	{#if isFormOpen}
		<div class="flashcard-form" role="region" aria-label={editingCardId ? 'Editar cartão' : 'Criar novo cartão'}>
			<div class="form-header">
				<span class="form-title">{editingCardId ? 'Editar Cartão' : 'Novo Cartão de Memorização'}</span>
				<span class="form-hint">Dica: pressione Ctrl+Enter para salvar, Esc para cancelar</span>
			</div>

			<div class="form-field">
				<label for="flashcard-front" class="field-label">Frente (Pergunta ou Conceito)</label>
				<!-- svelte-ignore a11y_autofocus -->
				<textarea
					id="flashcard-front"
					class="field-input"
					placeholder="Ex: O que significa o princípio da responsabilidade única?"
					rows="3"
					maxlength="4000"
					autofocus
					bind:value={frontInput}
				></textarea>
				<span class="char-count">{frontInput.trim().length} / 4000</span>
			</div>

			<div class="form-field">
				<label for="flashcard-back" class="field-label">Verso (Resposta ou Explicação)</label>
				<textarea
					id="flashcard-back"
					class="field-input"
					placeholder="Ex: Uma classe/módulo deve ter um e apenas um motivo para mudar."
					rows="3"
					maxlength="4000"
					bind:value={backInput}
				></textarea>
				<span class="char-count">{backInput.trim().length} / 4000</span>
			</div>

			<div class="form-actions">
				<button class="lv-btn lvui-btn lv-btn--outline" type="button" onclick={cancelForm} disabled={saving}>
					Cancelar
				</button>
				<button
					class="lv-btn lvui-btn"
					type="button"
					onclick={saveCard}
					disabled={saving || !frontInput.trim() || !backInput.trim()}
				>
					{saving ? 'Salvando…' : editingCardId ? 'Atualizar cartão' : 'Criar cartão'}
				</button>
			</div>
		</div>
	{/if}

	{#if loading}
		<p class="flashcards-status" role="status">Carregando cartões…</p>
	{:else if cards.length === 0 && !isFormOpen}
		<div class="flashcards-empty" role="region" aria-label="Nenhum cartão">
			<svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
				<rect x="2" y="4" width="20" height="16" rx="2" />
				<path d="M12 9v6m-3-3h6" />
			</svg>
			<p class="empty-title">Nenhum cartão criado para esta aula ainda</p>
			<p class="empty-desc">Crie perguntas e respostas para revisar conceitos-chave usando repetição espaçada.</p>
			<button class="lv-btn lvui-btn" type="button" onclick={startCreating}>
				Criar meu primeiro cartão
			</button>
		</div>
	{:else if viewMode === 'review'}
		<!-- Modo de Revisão Ativa -->
		{#if dueCards.length === 0}
			<div class="flashcards-all-done" role="status">
				<svg class="done-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<circle cx="12" cy="12" r="10" />
					<path d="m9 12 2 2 4-4" />
				</svg>
				<h4 class="done-title">Tudo em dia por aqui!</h4>
				<p class="done-desc">
					Todos os {cards.length} cartões desta aula foram revisados. Volte mais tarde quando o agendamento vencer para reforçar a memória.
				</p>
				<button class="lv-btn lvui-btn lv-btn--outline" type="button" onclick={() => (viewMode = 'manage')}>
					Ver todos os cartões cadastrados
				</button>
			</div>
		{:else if currentCard}
			<div class="review-box" role="region" aria-label="Sessão de revisão">
				<div class="review-meta">
					<span class="review-step">{dueCards.length === 1 ? '1 cartão pendente' : `${dueCards.length} cartões pendentes`}</span>
					<span class="review-interval">Intervalo atual: {currentCard.intervalDays} {currentCard.intervalDays === 1 ? 'dia' : 'dias'}</span>
				</div>

				<div class="review-card {revealed ? 'review-card--revealed' : ''}">
					<div class="card-side card-front">
						<span class="card-side-tag">FRENTE (Pergunta)</span>
						<p class="card-text">{currentCard.front}</p>
					</div>

					{#if revealed}
						<div class="card-divider" aria-hidden="true"></div>
						<div class="card-side card-back">
							<span class="card-side-tag card-side-tag--back">VERSO (Resposta)</span>
							<p class="card-text">{currentCard.back}</p>
						</div>
					{/if}
				</div>

				<div class="review-actions">
					{#if !revealed}
						<button
							class="lv-btn lvui-btn lv-btn--full"
							type="button"
							onclick={revealAnswer}
							title="Pressione Espaço ou clique para revelar"
						>
							Revelar resposta <span class="shortcut-hint">[Espaço]</span>
						</button>
					{:else}
						<div class="rating-buttons" role="group" aria-label="Avalie sua lembrança">
							<button
								class="rating-btn rating-btn--again"
								type="button"
								onclick={() => submitReview('again')}
								disabled={reviewing}
								title="Pressione 1 para repetir"
							>
								<span class="rating-label">Errei / Repetir</span>
								<span class="rating-sub">+10 min <span class="shortcut-tag">[1]</span></span>
							</button>

							<button
								class="rating-btn rating-btn--good"
								type="button"
								onclick={() => submitReview('good')}
								disabled={reviewing}
								title="Pressione 2 para marcar como bom"
							>
								<span class="rating-label">Acertei / Bom</span>
								<span class="rating-sub">{formatNextGoodInterval(currentCard)} <span class="shortcut-tag">[2]</span></span>
							</button>
						</div>
					{/if}
				</div>
			</div>
		{/if}
	{:else}
		<!-- Modo de Gerenciamento / Lista de Cartões -->
		<div class="manage-list" role="region" aria-label="Todos os cartões">
			{#each cards as card (card.cardId)}
				<article class="manage-card-item">
					<div class="manage-card-body">
						<div class="card-preview-section">
							<span class="preview-label">P:</span>
							<span class="preview-text">{card.front}</span>
						</div>
						<div class="card-preview-section card-preview-section--back">
							<span class="preview-label">R:</span>
							<span class="preview-text">{card.back}</span>
						</div>
						<div class="card-badges">
							<span class="card-badge {card.nextDue <= nowSeconds ? 'card-badge--due' : ''}">
								{formatDueBadge(card.nextDue)}
							</span>
							<span class="card-badge">
								Intervalo: {card.intervalDays} {card.intervalDays === 1 ? 'dia' : 'dias'}
							</span>
						</div>
					</div>

					<div class="manage-card-actions">
						{#if deleteConfirmId === card.cardId}
							<div class="delete-confirmation" role="alert">
								<span class="confirm-text">Excluir cartão?</span>
								<button class="lv-btn lvui-btn lv-btn--danger-sm" type="button" onclick={() => executeDelete(card.cardId)}>
									Confirmar
								</button>
								<button class="lv-btn lvui-btn lv-btn--subtle-sm" type="button" onclick={cancelDelete}>
									Não
								</button>
							</div>
						{:else}
							<button class="action-btn" type="button" onclick={() => startEditing(card)} title="Editar cartão">
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="action-icon" aria-hidden="true">
									<path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
									<path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
								</svg>
								Editar
							</button>
							<button class="action-btn action-btn--danger" type="button" onclick={() => requestDelete(card.cardId)} title="Excluir cartão">
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="action-icon" aria-hidden="true">
									<polyline points="3 6 5 6 21 6" />
									<path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
								</svg>
								Excluir
							</button>
						{/if}
					</div>
				</article>
			{/each}
		</div>
	{/if}
</section>

<style>
	@import './flashcards.css';
</style>
