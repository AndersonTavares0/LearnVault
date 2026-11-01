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

export type ForestStats = {
	totalTrees: number;
	healthyTrees: number;
	witheredTrees: number;
	successRate: number;
	totalMinutes: number;
};

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
	const healthyTrees = trees.filter((t) => t.completed).length;
	const witheredTrees = totalTrees - healthyTrees;
	const successRate = totalTrees > 0 ? Math.round((healthyTrees / totalTrees) * 100) : 100;
	const totalMinutes = trees.filter((t) => t.completed).reduce((sum, t) => sum + t.minutes, 0);
	return {
		totalTrees,
		healthyTrees,
		witheredTrees,
		successRate,
		totalMinutes
	};
}
