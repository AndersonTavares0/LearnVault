import { describe, expect, it } from 'vitest';
import { knowledgeTree } from './knowledge-tree';

const DAY = 86_400_000;
const NOW = new Date('2026-10-23T12:00:00Z').getTime();
const idle = (days: number) => NOW - days * DAY;

describe('knowledgeTree', () => {
	it('começa como semente sem atividade', () => {
		const tree = knowledgeTree(0, null, NOW);
		expect(tree.art).toBe('/knowledge-tree/seed-stage.svg');
		expect(tree.vitality).toBe('saudável');
		expect(tree.milestone).toBe('1 aula');
		expect(tree.vitalityHint).toBe('Conclua uma aula para brotar.');
	});

	it('avança de estágio nos marcos de aulas', () => {
		expect(knowledgeTree(1, idle(0), NOW).stageLabel).toBe('Broto · 1 aula concluída');
		expect(knowledgeTree(5, idle(0), NOW).art).toBe('/knowledge-tree/growth-stage.svg');
		expect(knowledgeTree(19, idle(0), NOW).milestone).toBe('20 aulas');
		expect(knowledgeTree(100, idle(0), NOW).milestone).toBe('Marco máximo alcançado');
		expect(knowledgeTree(150, idle(0), NOW).art).toBe('/knowledge-tree/tree-stage.svg');
	});

	it('murcha, enfraquece e morre por inatividade', () => {
		expect(knowledgeTree(6, idle(2), NOW)).toMatchObject({
			vitality: 'saudável',
			vitalityHint: '1 dia até murchar'
		});
		expect(knowledgeTree(6, idle(3), NOW)).toMatchObject({
			vitality: 'murchando',
			art: '/knowledge-tree/tree-wilting.svg',
			vitalityHint: '2 dias até enfraquecer'
		});
		expect(knowledgeTree(6, idle(5), NOW)).toMatchObject({
			vitality: 'enfraquecida',
			art: '/knowledge-tree/tree-weakened.svg',
			vitalityHint: '2 dias até morrer'
		});
		expect(knowledgeTree(6, idle(7), NOW)).toMatchObject({
			vitality: 'morta',
			art: '/knowledge-tree/tree-dead.svg',
			vitalityHint: 'Novo ciclo na próxima aula.'
		});
	});

	it('a semente não morre antes da primeira aula', () => {
		expect(knowledgeTree(0, idle(30), NOW)).toMatchObject({
			vitality: 'saudável',
			art: '/knowledge-tree/seed-stage.svg'
		});
	});

	it('ignora contagem negativa e calcula o restante', () => {
		expect(knowledgeTree(-3, idle(0), NOW).remaining).toBe('1 aula');
		expect(knowledgeTree(18, idle(0), NOW).remaining).toBe('2 aulas');
	});
});
