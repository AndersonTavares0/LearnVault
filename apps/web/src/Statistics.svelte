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
	<header class="lv-page__head">
		<div>
			<h1>Estatísticas</h1>
			<p class="lv-lede">Acompanhe suas aulas concluídas, sessões de foco e dias de estudo.</p>
		</div>
	</header>

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
		<section class="lv-panel lv-personal" aria-labelledby="personal-heading">
			<div class="lv-panel__head"><h2 id="personal-heading">Seu aprendizado</h2></div>
			{#if summaryLoading}
				<p class="lv-stats__loading" aria-live="polite">Carregando seu progresso…</p>
			{:else if summary && !rewardError}
				<dl class="lv-personal__metrics">
					<div><dt>Aulas concluídas</dt><dd>{summary.completedLessons}</dd></div>
					<div><dt>Sessões de foco</dt><dd>{focusError ? '—' : focus.totalSessions}</dd></div>
					<div>
						<dt>Dias ativos na semana</dt>
						<dd>{summary.weekActiveDays}</dd>
						<small>de segunda a domingo</small>
					</div>
				</dl>
			{:else}
				<p class="lv-stats__note" role="status">Não foi possível carregar seu progresso. Tente novamente na seção Sua evolução.</p>
			{/if}
		</section>
		<div class="lv-stats__overview">
			<section class="lv-panel lv-evolution" aria-labelledby="evolution-heading">
				<div class="lv-panel__head">
					<h2 id="evolution-heading">Sua evolução</h2>
					{#if gamification}
						<div class="lv-level-wrap">
							<span class="lv-level-chip">Nível {gamification.level.level}</span>
							<span class="lv-level-title">{gamification.level.title}</span>
						</div>
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
						<div class="lv-xp">
							<div class="lv-xp__head">
								<span><strong>{gamification.xp}</strong> XP <small>({Math.round(gamification.level.progress * 100)}%)</small></span>
								<span>{gamification.level.xpToNext} XP para o Nível {gamification.level.level + 1}</span>
							</div>
							<div class="lv-xp__track" role="progressbar" aria-label="Progresso para o próximo nível" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(gamification.level.progress * 100)}>
								<span class="lv-xp__fill" style:width={`${gamification.level.progress * 100}%`}></span>
							</div>
						</div>

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
							<p>{summary.weekActiveDays >= weeklyGoal ? 'Meta fechada. Continue no ritmo.' : `Faltam ${weeklyGoal - summary.weekActiveDays} ${weeklyGoal - summary.weekActiveDays === 1 ? 'dia' : 'dias'} para fechar a semana.`}</p>
							{#if goalError}<p class="lv-form-error" role="alert">{goalError}</p>{/if}
						</div>
						<div class="lv-streak">
							<div class="lv-streak__top">
								<div>
									<span>Ofensiva</span>
									<strong class="lv-streak__value">{summary.currentStreak} {summary.currentStreak === 1 ? 'dia' : 'dias'}</strong>
								</div>
								<p class="lv-streak__status" class:is-risk={summary.atRisk}>
									{summary.atRisk ? 'Em risco — estude hoje' : summary.todayActive ? 'Hoje conta' : 'Comece hoje'}
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
							<p class="lv-streak__record">Recorde: {summary.longestStreak} {summary.longestStreak === 1 ? 'dia' : 'dias'}</p>
						</div>

					</div>
				{/if}
			</section>

			<section class="lv-panel lv-focus" aria-labelledby="focus-heading">
				<div class="lv-panel__head">
					<h2 id="focus-heading">Foco</h2>
					<span class="lv-count">semana atual</span>
				</div>
				<div class="lv-focus__body">
					{#if summaryLoading}
						<p class="lv-stats__loading" aria-live="polite">Carregando estatísticas de foco…</p>
					{:else if focusError}
						<div class="lv-stats__loading"><p class="lv-stats__note" role="status">{focusError}</p><button class="lv-btn" type="button" onclick={loadSummary}>Tentar novamente</button></div>
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
		</div>

		<Forest />

		<div class="lv-stats__grid">
			<details class="lv-panel lv-library-inventory">
				<summary>Sua biblioteca em números</summary>
				<div class="lv-stats__metrics">
				<article class="lv-stat"><span>Cursos</span><strong>{stats.totals.courses}</strong><small>{formatCount(stats.totals.modules, 'módulo', 'módulos')}</small></article>
				<article class="lv-stat"><span>Aulas</span><strong>{stats.totals.lessons}</strong><small>{stats.totals.missingLessons > 0 ? formatCount(stats.totals.missingLessons, 'ausente', 'ausentes') : 'todas disponíveis'}</small></article>
				<article class="lv-stat"><span>Vídeos</span><strong>{stats.totals.videos - missingVideos}</strong><small>{missingVideos > 0 ? `${missingVideos} sem arquivo` : 'mídias no catálogo'}</small></article>
				<article class="lv-stat"><span>Materiais</span><strong>{stats.totals.materials}</strong><small>arquivos de apoio</small></article>
			</div>

				<section class="lv-library-volume" aria-labelledby="volume-heading">
					<div class="lv-panel__head"><h2 id="volume-heading">Cursos por volume</h2><span class="lv-count">aulas</span></div>
					<ol class="lv-volume">
						{#each stats.courses.slice(0, 6) as entry (courseIdOf(entry.course))}
							<li>
								<a class="lv-volume__link" href={href('course', courseIdOf(entry.course))}>
									<span class="lv-volume__head"><span>{entry.course.title}</span><small>{formatLessonCounts(entry.counts)}</small></span>
									<span class="lv-volume__track"><span class="lv-volume__bar" style:width={`${Math.max(6, (entry.counts.lessons / maxLessons) * 100)}%`}></span></span>
								</a>
							</li>
						{/each}
					</ol>
				</section>
			</details>

				<section class="lv-panel lv-stats__recent-panel" aria-labelledby="activity-heading">
					<div class="lv-panel__head"><h2 id="activity-heading">Atividade recente</h2><span class="lv-count">histórico recente</span></div>
					<div class="lv-activity__summary">
						<div><strong>{summary?.weekActiveDays ?? stats.recent.activeLast7Days}</strong><span>dias ativos</span></div>
						<div><strong>{stats.recent.completed}</strong><span>concluídas</span></div>
						<div><strong>{stats.recent.inProgress}</strong><span>em andamento</span></div>
					</div>
					<p class="lv-stats__note">Completudes e atividade consideram apenas o histórico recente.</p>
					{#if stats.activity.length > 0}
						<ul class="lv-activity">
							{#each stats.activity.slice(0, 8) as entry (entry.progress.mediaId)}
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
						<p class="lv-empty">Nenhuma atividade registrada ainda.</p>
					{/if}
				</section>
			</div>

		<section class="lv-stats__section lv-stats__achievements-section" aria-labelledby="achievements-heading">
			<div class="lv-section__head">
				<div>
					<h2 id="achievements-heading">Marcos de aprendizado</h2>
					<p class="lv-stats__section-intro">XP e selos acompanham aulas concluídas. Os passos diários são só um guia, sem bônus.</p>
				</div>
			</div>
			{#if gamification}
				<div class="lv-stats__achievements">
					<div class="lv-panel">
						<div class="lv-panel__head"><h2>Aulas concluídas</h2><span class="lv-count">{gamification.badges.filter((badge) => badge.earned).length} de {gamification.badges.length} marcos</span></div>
						<ul class="lv-badges">
							{#each gamification.badges as badge (badge.id)}
								<li class="lv-badge" class:is-earned={badge.earned}>
									<span class="lv-badge__seal" aria-hidden="true">{badge.earned ? '✓' : '·'}</span>
									<span class="lv-badge__body"><strong>{badge.title}</strong><small>{badge.description}</small></span>
									{#if !badge.earned}<span class="lv-badge__progress">{badge.progress}/{badge.target}</span>{/if}
								</li>
							{/each}
						</ul>
					</div>
					<div class="lv-panel">
						<div class="lv-panel__head"><h2>Passos de hoje</h2><span class="lv-count">progresso diário</span></div>
						<ul class="lv-missions">
							{#each gamification.missions as mission (mission.id)}
								<li class="lv-mission" class:is-done={mission.progress >= mission.target}>
									<div class="lv-mission__head"><strong>{mission.title}</strong><span>{mission.progress}/{mission.target}</span></div>
									<p>{mission.description}</p>
									<div class="lv-mission__track" role="progressbar" aria-label={`Progresso: ${mission.title}`} aria-valuemin="0" aria-valuemax={mission.target} aria-valuenow={mission.progress}>
										<span style:width={`${Math.min(100, (mission.progress / mission.target) * 100)}%`}></span>
									</div>
									<div class="lv-mission__foot">
										<span>{mission.claimed ? 'Recompensa resgatada' : mission.progress >= mission.target ? `Recompensa disponível · +${mission.rewardXp} XP` : 'Continue no seu ritmo'}</span>
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
					</div>
				</div>
			{:else}
				<p class="lv-empty">Carregando conquistas…</p>
			{/if}
		</section>

		<p class="lv-stats__foot">{formatCount(stats.courses.length, 'curso', 'cursos')} no acervo · Seu progresso é pessoal; dias ativos seguem o fuso local do servidor ou do dispositivo.</p>
	{/if}
</div>

<style>
	.lv-level-wrap {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.lv-level-title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--muted);
	}
</style>
