export type TreeVitality = 'saudável' | 'murchando' | 'enfraquecida' | 'morta';

export type KnowledgeTree = {
	art: string;
	alt: string;
	stageLabel: string;
	milestone: string;
	remaining: string;
	vitality: TreeVitality;
	vitalityHint: string;
};

const DAY = 86_400_000;

const STAGES = [
	{ at: 0, label: 'Semente', art: 'seed-stage.svg' },
	{ at: 1, label: 'Broto', art: 'sprout-stage.svg' },
	{ at: 5, label: 'Muda', art: 'growth-stage.svg' },
	{ at: 20, label: 'Árvore', art: 'tree-stage.svg' },
	{ at: 50, label: 'Árvore desenvolvida', art: 'tree-stage.svg' },
	{ at: 100, label: 'Árvore majestosa', art: 'tree-stage.svg' }
] as const;

const dayLabel = (days: number) => `${days} ${days === 1 ? 'dia' : 'dias'}`;
const lessonLabel = (count: number) => `${count} ${count === 1 ? 'aula' : 'aulas'}`;

/** Deriva estágio e vitalidade só do que a API expõe: aulas concluídas e última atividade. */
export function knowledgeTree(
	completedCount: number,
	lastActivityAt: number | null,
	now = Date.now()
): KnowledgeTree {
	const count = Math.max(0, Math.floor(completedCount));
	let stage: (typeof STAGES)[number] = STAGES[0];
	for (const item of STAGES) if (count >= item.at) stage = item;
	const next = STAGES.find((item) => item.at > count);
	const idleDays =
		lastActivityAt === null ? 0 : Math.max(0, Math.floor((now - lastActivityAt) / DAY));

	let vitality: TreeVitality = 'saudável';
	let healthArt: string | null = null;
	let vitalityHint = `${dayLabel(3 - Math.min(idleDays, 2))} até murchar`;

	if (count === 0) {
		vitalityHint = 'Conclua uma aula para brotar.';
	} else if (idleDays >= 7) {
		vitality = 'morta';
		healthArt = 'tree-dead.svg';
		vitalityHint = 'Novo ciclo na próxima aula.';
	} else if (idleDays >= 5) {
		vitality = 'enfraquecida';
		healthArt = 'tree-weakened.svg';
		vitalityHint = `${dayLabel(7 - idleDays)} até morrer`;
	} else if (idleDays >= 3) {
		vitality = 'murchando';
		healthArt = 'tree-wilting.svg';
		vitalityHint = `${dayLabel(5 - idleDays)} até enfraquecer`;
	}

	return {
		art: `/knowledge-tree/${healthArt ?? stage.art}`,
		alt: `${stage.label} ${vitality} da Árvore do conhecimento`,
		stageLabel: `${stage.label} · ${count === 1 ? '1 aula concluída' : `${count} aulas concluídas`}`,
		milestone: next ? lessonLabel(next.at) : 'Marco máximo alcançado',
		remaining: next ? lessonLabel(next.at - count) : 'Nenhuma',
		vitality,
		vitalityHint
	};
}
