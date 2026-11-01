<script lang="ts">
	import { onMount } from 'svelte';
	import { formatCount, formatLessonCounts, countMissingVideos } from './catalog';
	import { buildGamification } from './gamification';
	import { localDayKey, weekDayKeys } from './pomodoro';
	import { formatDuration } from './player';
	import { href } from './router';
	import { buildStatistics, weeklyGoalProgress } from './statistics';
	import { courseIdOf, transport, type Catalog, type DailyMission, type MissionId, type StudyProgress } from './transport';
	import { persistedFocusSummary, persistedRewardSummary } from './pomodoro-store';
	import Forest from './Forest.svelte';
	import LevelCrest from './LevelCrest.svelte';

	let { catalog, recent }: { catalog: Catalog | null; recent: StudyProgress[] } = $props();

	const stats = $derived(buildStatistics(catalog, recent));
	const maxLessons = $derived(Math.max(1, ...stats.courses.map((entry) => entry.counts.lessons)));
	const missingVideos = $derived((catalog?.nodes ?? []).reduce((sum, node) => sum + countMissingVideos(node), 0));
	const dateFormat = new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: 'short' });
	const weekLabels = ['seg', 'ter', 'qua', 'qui', 'sex', 'sáb', 'dom'];

	let summary = $derived($persistedRewardSummary?.study ?? null);
	let rewards = $derived($persistedRewardSummary);
	let focus = $derived($persistedFocusSummary ?? {
		todayMinutes: 0, weekMinutes: 0, weekSessions: 0, totalMinutes: 0, totalSessions: 0,
		completionRate: 0, bestDay: null, days: weekDayKeys().map((day) => ({ day, minutes: 0, sessions: 0 }))
	});
	let summaryLoading = $state(true);
	let rewardError = $state('');
	let focusError = $state('');
	let weeklyGoal = $state(3);
	let goalSaving = $state(false);
	let goalError = $state('');
	let missionError = $state('');
	let claimingMission = $state<MissionId | null>(null);

	const maxFocusMinutes = $derived(Math.max(1, ...focus.days.map((day) => day.minutes)));
	const bestFocusDay = $derived(focus.bestDay ? focus.days.find((day) => day.day === focus.bestDay) ?? null : null);
	const today = $derived(summary?.today ?? localDayKey(new Date()));
	const goalProgress = $derived(summary ? weeklyGoalProgress(summary.weekActiveDays, weeklyGoal) : 0);
	const gamification = $derived(rewards ? buildGamification({ rewards }) : null);
	const hasActivity = $derived(Boolean(
		(summary && (summary.completedLessons > 0 || summary.currentStreak > 0 || summary.weekActiveDays > 0)) ||
		focus.totalSessions > 0 ||
		(gamification && gamification.xp > 0)
	));

	onMount(() => {
		void loadSummary();
	});

	async function loadSummary() {
		summaryLoading = true;
		rewardError = '';
		focusError = '';
		const [rewardResult, focusResult] = await Promise.allSettled([transport.rewardSummary(), transport.focusSummary()]);
		if (rewardResult.status === 'fulfilled') persistedRewardSummary.set(rewardResult.value);
		else rewardError = rewardResult.reason instanceof Error ? rewardResult.reason.message : 'Não foi possível carregar seus dados de evolução.';
		if (focusResult.status === 'fulfilled') persistedFocusSummary.set(focusResult.value);
		else focusError = focusResult.reason instanceof Error ? focusResult.reason.message : 'Não foi possível carregar as estatísticas de foco.';
		summaryLoading = false;
	}

	$effect(() => {
		const savedGoal = rewards?.weeklyGoal;
		if (savedGoal && savedGoal !== weeklyGoal) weeklyGoal = savedGoal;
	});

	async function changeGoal(event: Event) {
		const requested = Number((event.currentTarget as HTMLSelectElement).value) || 3;
		const previous = weeklyGoal;
		weeklyGoal = requested;
		goalError = '';
		goalSaving = true;
		try {
			weeklyGoal = await transport.setWeeklyGoal(requested);
			await loadSummary();
		} catch (cause) {
			weeklyGoal = previous;
			goalError = cause instanceof Error ? cause.message : 'Não foi possível salvar a meta semanal.';
		} finally {
			goalSaving = false;
		}
	}

	async function claimMission(mission: DailyMission) {
		if (mission.claimed || mission.progress < mission.target || claimingMission) return;
		missionError = '';
		claimingMission = mission.id;
		try {
			await transport.claimDailyMission(mission.id);
			await loadSummary();
		} catch (cause) {
			missionError = cause instanceof Error ? cause.message : 'Não foi possível resgatar a missão.';
		} finally {
			claimingMission = null;
		}
	}

	function when(seconds: number | null): string {
		if (seconds === null) return '';
		return dateFormat.format(new Date(seconds * 1000));
	}

	function dayLabel(day: string): string {
		const weekday = new Date(`${day}T12:00:00`).getDay();
		return weekLabels[(weekday + 6) % 7] ?? '';
	}
</script>

<svelte:head>
	<title>Estatísticas — LearnVault</title>
	<meta name="description" content="Seu estudo, seu foco e sua evolução no LearnVault." />
</svelte:head>

<div class="lv-page lv-stats" class:is-empty={!summaryLoading && !rewardError && !focusError && !!summary && !hasActivity}>
	<!-- Cabeçalho Executivo do Hub de Estatísticas -->
	<header class="lv-page__head lv-stats__header">
		<div>
			<h1>Estatísticas</h1>
			<p class="lv-lede">Acompanhe seu ritmo de estudo, sua evolução pessoal e o florescimento da sua floresta.</p>
		</div>

		<!-- Badges de Status Rápido no Topo -->
		<div class="lv-stats__quick-pills" role="region" aria-label="Resumo do perfil">
			{#if gamification}
				<div class="lv-quick-pill" title="Nível atual e XP acumulado">
					<LevelCrest level={gamification.level.level} size={20} />
					<span>Nível {gamification.level.level} · {gamification.level.title}</span>
					<strong class="lv-quick-pill__highlight">{gamification.xp} XP</strong>
				</div>
			{/if}
			{#if summary}
				<div class="lv-quick-pill" class:is-risk={summary.atRisk} title="Ofensiva diária de estudo">
					<span aria-hidden="true">🔥</span>
					<span>Ofensiva:</span>
					<strong>{summary.currentStreak} {summary.currentStreak === 1 ? 'dia' : 'dias'}</strong>
				</div>
			{/if}
			<div class="lv-quick-pill" title="Tempo total dedicado ao foco">
				<span aria-hidden="true">⏱️</span>
				<span>Foco:</span>
				<strong>{focus.totalMinutes} min</strong>
			</div>
		</div>
	</header>

	{#if rewardError}
		<div class="lv-stats__alert-banner" role="alert">
			<div class="lv-stats__alert-text">
				<span aria-hidden="true">⚠️</span>
				<span>{rewardError}</span>
			</div>
			<button class="lv-btn lv-btn--small" type="button" onclick={loadSummary}>Tentar novamente</button>
		</div>
	{/if}

	{#if !catalog || catalog.nodes.length === 0}
		<div class="lv-empty-state">
			<p class="lv-empty__title">Ainda não há o que medir</p>
			<p class="lv-lede">Importe um curso para começar a acompanhar aulas, foco e ofensiva.</p>
			<a class="lv-btn" href={href('home')}>Voltar à biblioteca</a>
		</div>
	{:else}
		{#if !summaryLoading && !rewardError && !focusError && summary && !hasActivity}
			<section class="lv-panel lv-stats-empty" aria-labelledby="start-heading">
				<h2 id="start-heading">Seu histórico começa na primeira aula</h2>
				<p>Conclua uma aula ou inicie uma sessão de foco pelo controle no topo. Os números aparecem aqui quando houver atividade.</p>
				<a class="lv-btn lv-btn--primary" href={href('home')}>Abrir a biblioteca</a>
			</section>
		{/if}

		<!-- 1. HERO EM LARGURA TOTAL: A FLORESTA DE ESTUDOS (Estilo Forest App) -->
		<div class="lv-stats__hero-forest">
			<Forest />
		</div>

		<!-- 2. DASHBOARD DE DESEMPENHO E EVOLUÇÃO (2 Colunas Equilibradas) -->
		<div class="lv-stats__dashboard-grid">
			<!-- COLUNA ESQUERDA: FOCO & AULAS -->
			<div class="lv-stats__col">
				<!-- Ritmo de Foco Semanal -->
				<section class="lv-panel lv-focus" aria-labelledby="focus-heading">
					<div class="lv-panel__head">
						<h2 id="focus-heading">Ritmo de foco na semana</h2>
						<span class="lv-count">semana atual</span>
					</div>
					<div class="lv-focus__body">
						{#if summaryLoading}
							<p class="lv-stats__loading" aria-live="polite">Carregando estatísticas de foco…</p>
						{:else if focusError}
							<div class="lv-stats__loading">
								<p class="lv-stats__note" role="status">{focusError}</p>
								<button class="lv-btn" type="button" onclick={loadSummary}>Tentar novamente</button>
							</div>
						{:else}
							<div class="lv-focus__stats">
								<div><strong>{focus.todayMinutes}</strong><span>min hoje</span></div>
								<div><strong>{focus.weekMinutes}</strong><span>min na semana</span></div>
								<div><strong>{focus.weekSessions}</strong><span>sessões</span></div>
								<div><strong>{focus.completionRate}%</strong><span>concluídas</span></div>
							</div>
							<ol class="lv-focus__week" aria-label="Minutos de foco por dia">
								{#each focus.days as day, index (day.day)}
									<li class:is-active={day.minutes > 0} class:is-today={day.day === today}>
										<span class="lv-focus__bar" style:height={`${Math.max(2, (day.minutes / maxFocusMinutes) * 100)}%`}></span>
										<span class="lv-focus__label">{weekLabels[index]}</span>
										<span class="visually-hidden">{day.minutes} minutos</span>
									</li>
								{/each}
							</ol>
							<p class="lv-focus__best">{bestFocusDay ? `Melhor dia: ${dayLabel(bestFocusDay.day)} · ${bestFocusDay.minutes} min` : 'Sem sessões nesta semana ainda.'}</p>
						{/if}
					</div>
				</section>

				<!-- Atividade Recente -->
				<section class="lv-panel lv-stats__recent-panel" aria-labelledby="activity-heading">
					<div class="lv-panel__head">
						<h2 id="activity-heading">Atividade recente</h2>
						<span class="lv-count">aulas</span>
					</div>
					<div class="lv-activity__summary">
						<div><strong>{summary?.weekActiveDays ?? stats.recent.activeLast7Days}</strong><span>dias ativos</span></div>
						<div><strong>{stats.recent.completed}</strong><span>concluídas</span></div>
						<div><strong>{stats.recent.inProgress}</strong><span>em andamento</span></div>
					</div>
					{#if stats.activity.length > 0}
						<ul class="lv-activity">
							{#each stats.activity.slice(0, 6) as entry (entry.progress.mediaId)}
								<li>
									<a class="lv-activity__main" href={href('course', entry.location.courseId, entry.location.lesson.path)}>
										<span class="lv-activity__lesson">{entry.location.lesson.title}</span>
										<small class="lv-activity__course">{entry.location.courseTitle}{entry.location.moduleTitle ? ` · ${entry.location.moduleTitle}` : ''}</small>
									</a>
									<span class="lv-activity__status">{entry.progress.completed ? 'Concluída' : entry.progress.positionSeconds > 0 ? `Parou em ${formatDuration(entry.progress.positionSeconds)}` : 'Não iniciada'}</span>
									<time class="lv-activity__time" datetime={entry.progress.updatedAt ? new Date(entry.progress.updatedAt * 1000).toISOString() : undefined}>{when(entry.progress.updatedAt)}</time>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="lv-empty">Nenhuma atividade recente registrada ainda.</p>
					{/if}
				</section>
			</div>

			<!-- COLUNA DIREITA: EVOLUÇÃO & HÁBITOS -->
			<div class="lv-stats__col">
				<!-- Card de Evolução Pessoal -->
				<section class="lv-panel lv-evolution" aria-labelledby="evolution-heading">
					<div class="lv-panel__head">
						<h2 id="evolution-heading">Sua evolução</h2>
						{#if gamification}
							<span class="lv-level-chip">Nível {gamification.level.level}</span>
						{/if}
					</div>

					{#if summaryLoading}
						<div class="lv-stats__loading" aria-live="polite">Carregando sua evolução…</div>
					{:else if rewardError}
						<div class="lv-stats__loading">
							<p>{rewardError}</p>
							<button class="lv-btn" type="button" onclick={loadSummary}>Tentar de novo</button>
						</div>
					{:else if summary && gamification}
						<div class="lv-evolution__body">
							<!-- Barra de XP -->
							<div class="lv-xp">
								<div class="lv-xp__head">
									<span><strong>{gamification.xp}</strong> XP <small>({Math.round(gamification.level.progress * 100)}%)</small></span>
									<span>{gamification.level.xpToNext} XP para o Nível {gamification.level.level + 1}</span>
								</div>
								<div class="lv-xp__track" role="progressbar" aria-label="Progresso para o próximo nível" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(gamification.level.progress * 100)}>
									<span class="lv-xp__fill" style:width={`${gamification.level.progress * 100}%`}></span>
								</div>
							</div>

							<!-- Meta Semanal -->
							<div class="lv-goal">
								<div class="lv-goal__header">
									<label for="weekly-goal">Meta semanal</label>
									<select class="lvui-select" id="weekly-goal" value={weeklyGoal} onchange={changeGoal} disabled={goalSaving}>
										{#each [1, 2, 3, 4, 5, 6, 7] as option}
											<option value={option}>{option} {option === 1 ? 'dia' : 'dias'}</option>
										{/each}
									</select>
								</div>
								<div class="lv-goal__progress">
									<strong>{Math.min(summary.weekActiveDays, weeklyGoal)} de {weeklyGoal} dias</strong>
									<span>{Math.round((goalProgress / Math.max(1, weeklyGoal)) * 100)}%</span>
								</div>
								<div class="lv-goal__track" role="progressbar" aria-label="Progresso da meta semanal" aria-valuemin="0" aria-valuemax={weeklyGoal} aria-valuenow={goalProgress}>
									<span class="lv-goal__bar" style:width={`${(goalProgress / Math.max(1, weeklyGoal)) * 100}%`}></span>
								</div>
								<p class="lv-goal__hint">{summary.weekActiveDays >= weeklyGoal ? 'Meta fechada. Continue no ritmo.' : `Faltam ${weeklyGoal - summary.weekActiveDays} ${weeklyGoal - summary.weekActiveDays === 1 ? 'dia' : 'dias'} para fechar a semana.`}</p>
								{#if goalError}<p class="lv-form-error" role="alert">{goalError}</p>{/if}
							</div>

							<!-- Ofensiva Diária -->
							<div class="lv-streak">
								<div class="lv-streak__top">
									<div>
										<span>Ofensiva</span>
										<strong class="lv-streak__value">{summary.currentStreak} {summary.currentStreak === 1 ? 'dia' : 'dias'}</strong>
									</div>
									<p class="lv-streak__status" class:is-risk={summary.atRisk}>
										{summary.atRisk ? 'Em risco — estude hoje' : summary.todayActive ? 'Hoje ativo ✓' : 'Comece hoje'}
									</p>
								</div>
								<ol class="lv-streak__days">
									{#each summary.days as day, index (day.day)}
										<li class="lv-streak__day" class:is-active={day.active} class:is-today={day.day === today} title={`${weekLabels[index]}: ${day.active ? 'ativo' : 'sem atividade'}`}>
											<span aria-hidden="true">{weekLabels[index]}</span>
											<span class="lv-streak__dot" aria-hidden="true"></span>
											<span class="visually-hidden">{day.day}: {day.active ? 'ativo' : 'sem atividade'}</span>
										</li>
									{/each}
								</ol>
								<p class="lv-streak__record">Recorde pessoal: {summary.longestStreak} {summary.longestStreak === 1 ? 'dia' : 'dias'}</p>
							</div>
						</div>
					{/if}
				</section>

				<!-- Passos de Hoje (Missões Diárias de XP) -->
				{#if gamification}
					<section class="lv-panel lv-daily-missions" aria-labelledby="missions-heading">
						<div class="lv-panel__head">
							<h2 id="missions-heading">Passos de hoje</h2>
							<span class="lv-count">missões diárias</span>
						</div>
						<ul class="lv-missions">
							{#each gamification.missions as mission (mission.id)}
								<li class="lv-mission" class:is-done={mission.progress >= mission.target}>
									<div class="lv-mission__head"><strong>{mission.title}</strong><span>{mission.progress}/{mission.target}</span></div>
									<p>{mission.description}</p>
									<div class="lv-mission__track" role="progressbar" aria-label={`Progresso: ${mission.title}`} aria-valuemin="0" aria-valuemax={mission.target} aria-valuenow={mission.progress}>
										<span style:width={`${Math.min(100, (mission.progress / Math.max(1, mission.target)) * 100)}%`}></span>
									</div>
									<div class="lv-mission__foot">
										<span>{mission.claimed ? 'Recompensa resgatada' : mission.progress >= mission.target ? `Disponível · +${mission.rewardXp} XP` : 'Continue no seu ritmo'}</span>
										{#if mission.claimed}
											<span class="lv-count">Resgatada</span>
										{:else if mission.progress >= mission.target}
											<button class="lv-btn lv-btn--primary" type="button" disabled={claimingMission !== null} onclick={() => void claimMission(mission)}>{claimingMission === mission.id ? 'Resgatando…' : 'Resgatar'}</button>
										{/if}
									</div>
								</li>
							{/each}
						</ul>
						{#if missionError}<p class="lv-form-error" role="alert">{missionError}</p>{/if}
					</section>

					<!-- Marcos de Aulas Concluídas -->
					<section class="lv-panel lv-badges-panel" aria-labelledby="badges-heading">
						<div class="lv-panel__head">
							<h2 id="badges-heading">Marcos de estudo</h2>
							<span class="lv-count">{gamification.badges.filter((b) => b.earned).length} de {gamification.badges.length}</span>
						</div>
						<ul class="lv-badges">
							{#each gamification.badges as badge (badge.id)}
								<li class="lv-badge" class:is-earned={badge.earned}>
									<span class="lv-badge__seal" aria-hidden="true">{badge.earned ? '✓' : '·'}</span>
									<span class="lv-badge__body"><strong>{badge.title}</strong><small>{badge.description}</small></span>
									{#if !badge.earned}<span class="lv-badge__progress">{badge.progress}/{badge.target}</span>{/if}
								</li>
							{/each}
						</ul>
					</section>
				{/if}
			</div>
		</div>

		<!-- SEÇÃO INFERIOR: BIBLIOTECA EM NÚMEROS -->
		<section class="lv-panel lv-library-inventory" aria-labelledby="inventory-heading">
			<div class="lv-panel__head">
				<h2 id="inventory-heading">Sua biblioteca em números</h2>
				<span class="lv-count">{formatCount(stats.courses.length, 'curso', 'cursos')}</span>
			</div>
			<div class="lv-stats__metrics">
				<article class="lv-stat"><span>Cursos</span><strong>{stats.totals.courses}</strong><small>{formatCount(stats.totals.modules, 'módulo', 'módulos')}</small></article>
				<article class="lv-stat"><span>Aulas</span><strong>{stats.totals.lessons}</strong><small>{stats.totals.missingLessons > 0 ? formatCount(stats.totals.missingLessons, 'ausente', 'ausentes') : 'disponíveis'}</small></article>
				<article class="lv-stat"><span>Vídeos</span><strong>{stats.totals.videos - missingVideos}</strong><small>{missingVideos > 0 ? `${missingVideos} sem arquivo` : 'mídias no acervo'}</small></article>
				<article class="lv-stat"><span>Materiais</span><strong>{stats.totals.materials}</strong><small>arquivos de apoio</small></article>
			</div>

			<div class="lv-library-volume">
				<div class="lv-panel__head"><h3 style="font-size: 0.95rem; font-weight: 600;">Cursos por volume</h3><span class="lv-count">aulas</span></div>
				<ol class="lv-volume">
					{#each stats.courses.slice(0, 5) as entry (courseIdOf(entry.course))}
						<li>
							<a class="lv-volume__link" href={href('course', courseIdOf(entry.course))}>
								<span class="lv-volume__head"><span>{entry.course.title}</span><small>{formatLessonCounts(entry.counts)}</small></span>
								<span class="lv-volume__track"><span class="lv-volume__bar" style:width={`${Math.max(6, (entry.counts.lessons / maxLessons) * 100)}%`}></span></span>
							</a>
						</li>
					{/each}
				</ol>
			</div>
		</section>

		<p class="lv-stats__foot">{formatCount(stats.courses.length, 'curso', 'cursos')} no acervo · Seu progresso é pessoal; dias ativos seguem o fuso local do servidor ou do dispositivo.</p>
	{/if}
</div>

<style>
	.lv-stats__header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1.25rem;
		flex-wrap: wrap;
	}

	.lv-stats__quick-pills {
		display: flex;
		align-items: center;
		gap: 0.65rem;
		flex-wrap: wrap;
	}

	.lv-quick-pill {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: 9999px;
		padding: 0.35rem 0.85rem;
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--muted);
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
		transition: border-color 0.15s ease, background-color 0.15s ease;
	}

	.lv-quick-pill:hover {
		border-color: color-mix(in srgb, var(--accent) 35%, var(--line));
	}

	.lv-quick-pill strong {
		color: var(--ink);
		font-weight: 700;
	}

	.lv-quick-pill__highlight {
		color: var(--accent) !important;
	}

	.lv-quick-pill.is-risk {
		border-color: color-mix(in srgb, var(--warn, #b91c1c) 35%, var(--line));
		color: var(--warn, #b91c1c);
	}

	/* Hero da Floresta em Destaque (Full Width Atelier) */
	.lv-stats__hero-forest {
		width: 100%;
		margin-top: 0.25rem;
	}

	/* Dashboard de Foco & Evolução em 2 Colunas */
	.lv-stats__dashboard-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
		align-items: start;
		margin-top: 1rem;
	}

	.lv-stats__col {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		min-width: 0;
	}

	.lv-stats__alert-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		background: color-mix(in srgb, #f59e0b 10%, var(--surface));
		border: 1px solid color-mix(in srgb, #f59e0b 35%, var(--line));
		border-radius: var(--radius);
		padding: 0.75rem 1.25rem;
		margin-top: 0.5rem;
		margin-bottom: 0.5rem;
	}

	.lv-stats__alert-text {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.875rem;
		color: var(--ink);
	}

	.lv-goal__hint {
		font-size: 0.8125rem;
		color: var(--muted);
		margin-top: 0.35rem;
	}

	@media (max-width: 1024px) {
		.lv-stats__dashboard-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 768px) {
		.lv-stats__header {
			flex-direction: column;
			gap: 1rem;
		}

		.lv-stats__quick-pills {
			width: 100%;
		}
	}
</style>
