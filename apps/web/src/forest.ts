import type { FocusSession } from './pomodoro';

export type TreeStage = 'sapling' | 'oak' | 'pine' | 'ancient' | 'withered';

export type ForestTree = {
	sessionId: string;
	stage: TreeStage;
	stageName: string;
	minutes: number;
	completed: boolean;
	timestamp: number;
	dateIso: string;
	label: string;
};

export type ForestEcosystem = {
	level: number;
	name: string;
	description: string;
	currentCount: number;
	nextThreshold: number | null;
	progress: number;
	treesToNext: number;
	nextName: string | null;
};

export type ForestStats = {
	totalTrees: number;
	healthyTrees: number;
	witheredTrees: number;
	successRate: number;
	totalMinutes: number;
	ecosystem: ForestEcosystem;
	speciesCounts: Record<TreeStage, number>;
	longestSessionMinutes: number;
};

export const ECOSYSTEM_STAGES = [
	{
		min: 0,
		max: 0,
		level: 0,
		name: 'Clareira Inicial',
		description: 'Sua jornada de foco está pronta para fincar as primeiras raízes.',
		nextName: 'Bosque Jovem',
		nextThreshold: 1
	},
	{
		min: 1,
		max: 4,
		level: 1,
		name: 'Bosque Jovem',
		description: 'Primeiras mudas florescendo. O hábito de foco diário começa a ganhar forma.',
		nextName: 'Bosque Frondoso',
		nextThreshold: 5
	},
	{
		min: 5,
		max: 11,
		level: 2,
		name: 'Bosque Frondoso',
		description: 'Copas se entrelaçando. A disciplina diária ganha consistência e maturidade.',
		nextName: 'Floresta Verdejante',
		nextThreshold: 12
	},
	{
		min: 12,
		max: 24,
		level: 3,
		name: 'Floresta Verdejante',
		description: 'Um refúgio verde abundante nutrido por horas regulares de dedicação.',
		nextName: 'Reserva Natural',
		nextThreshold: 25
	},
	{
		min: 25,
		max: 49,
		level: 4,
		name: 'Reserva Natural',
		description: 'Ecossistema resiliente e diversificado, fruto de foco profundo continuado.',
		nextName: 'Santuário Ancestral',
		nextThreshold: 50
	},
	{
		min: 50,
		max: Infinity,
		level: 5,
		name: 'Santuário Ancestral',
		description: 'Mestria suprema de concentração. Um monumento vivo ao conhecimento acumulado.',
		nextName: null,
		nextThreshold: null
	}
];

export function deriveEcosystem(healthyTreesCount: number): ForestEcosystem {
	const count = Math.max(0, healthyTreesCount);
	const stage =
		ECOSYSTEM_STAGES.find((s) => count >= s.min && count <= s.max) ??
		ECOSYSTEM_STAGES[ECOSYSTEM_STAGES.length - 1];

	if (stage.nextThreshold === null) {
		return {
			level: stage.level,
			name: stage.name,
			description: stage.description,
			currentCount: count,
			nextThreshold: null,
			progress: 1,
			treesToNext: 0,
			nextName: null
		};
	}

	const span = stage.nextThreshold - stage.min;
	const currentInSpan = count - stage.min;
	const progress = Math.min(1, Math.max(0, currentInSpan / span));
	const treesToNext = Math.max(0, stage.nextThreshold - count);

	return {
		level: stage.level,
		name: stage.name,
		description: stage.description,
		currentCount: count,
		nextThreshold: stage.nextThreshold,
		progress,
		treesToNext,
		nextName: stage.nextName
	};
}

export type ForestFilter = 'today' | 'week' | 'month' | 'all';

export function deriveTreeStage(actualSeconds: number, completed: boolean): TreeStage {
	if (!completed) return 'withered';
	const minutes = Math.round(actualSeconds / 60);
	if (minutes <= 20) return 'sapling';
	if (minutes <= 40) return 'oak';
	if (minutes <= 60) return 'pine';
	return 'ancient';
}

export function treeStageName(stage: TreeStage): string {
	switch (stage) {
		case 'sapling':
			return 'Brotinho';
		case 'oak':
			return 'Carvalho Frondoso';
		case 'pine':
			return 'Pinheiro Nobre';
		case 'ancient':
			return 'Árvore Ancestral';
		case 'withered':
			return 'Árvore Seca';
	}
}

export function formatTreeLabel(tree: Omit<ForestTree, 'label'>): string {
	const date = new Date(tree.timestamp * 1000).toLocaleDateString('pt-BR');
	if (!tree.completed) {
		return `${tree.stageName}: sessão de ${tree.minutes} min interrompida em ${date}`;
	}
	return `${tree.stageName}: ${tree.minutes} min de foco cultivados em ${date}`;
}

export function buildForest(sessions: FocusSession[]): ForestTree[] {
	// Apenas sessões de foco entram na floresta (pausas não cultivam árvores).
	const focusSessions = sessions.filter((s) => s.kind === 'focus');
	return focusSessions.map((session) => {
		const stage = deriveTreeStage(session.actualSeconds, session.completed);
		const stageName = treeStageName(stage);
		const minutes = Math.max(1, Math.round(session.actualSeconds / 60));
		const timestamp = session.endedAt;
		const date = new Date(timestamp * 1000);
		const dateIso = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
		const partial = {
			sessionId: session.id,
			stage,
			stageName,
			minutes,
			completed: session.completed,
			timestamp,
			dateIso
		};
		return {
			...partial,
			label: formatTreeLabel(partial)
		};
	});
}

export function filterForest(trees: ForestTree[], range: ForestFilter, now = new Date()): ForestTree[] {
	if (range === 'all') return trees;
	const todayIso = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
	if (range === 'today') {
		return trees.filter((tree) => tree.dateIso === todayIso);
	}
	const nowSeconds = Math.floor(now.getTime() / 1000);
	if (range === 'week') {
		const sevenDaysAgo = nowSeconds - 7 * 86400;
		return trees.filter((tree) => tree.timestamp >= sevenDaysAgo);
	}
	if (range === 'month') {
		const thirtyDaysAgo = nowSeconds - 30 * 86400;
		return trees.filter((tree) => tree.timestamp >= thirtyDaysAgo);
	}
	return trees;
}

export function calculateForestStats(trees: ForestTree[]): ForestStats {
	const totalTrees = trees.length;
	const healthyTreesList = trees.filter((t) => t.completed);
	const healthyTrees = healthyTreesList.length;
	const witheredTrees = totalTrees - healthyTrees;
	const successRate = totalTrees > 0 ? Math.round((healthyTrees / totalTrees) * 100) : 100;
	const totalMinutes = healthyTreesList.reduce((sum, t) => sum + t.minutes, 0);

	const speciesCounts: Record<TreeStage, number> = {
		sapling: 0,
		oak: 0,
		pine: 0,
		ancient: 0,
		withered: 0
	};
	let longestSessionMinutes = 0;

	for (const tree of trees) {
		speciesCounts[tree.stage] = (speciesCounts[tree.stage] ?? 0) + 1;
		if (tree.completed && tree.minutes > longestSessionMinutes) {
			longestSessionMinutes = tree.minutes;
		}
	}

	const ecosystem = deriveEcosystem(healthyTrees);

	return {
		totalTrees,
		healthyTrees,
		witheredTrees,
		successRate,
		totalMinutes,
		ecosystem,
		speciesCounts,
		longestSessionMinutes
	};
}
