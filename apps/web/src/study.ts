import type { StudyProgress } from './transport';

export type SaveResult =
	| { kind: 'saved'; progress: StudyProgress }
	| { kind: 'stale' }
	| { kind: 'error'; message: string }
	| { kind: 'queued' };

/**
 * Cria um salvador de posição que serializa as requisições e coalesca múltiplas
 * atualizações enquanto uma requisição está em voo.
 *
 * Isso evita duas condições de corrida comuns no player:
 * 1. Duas requisições paralelas de `recordPosition` e a resposta mais antiga chega
 *    por último, sobrescrevendo o progresso com uma posição stale.
 * 2. Vários eventos `timeupdate` disparam requisições sucessivas, criando carga
 *    desnecessária no servidor.
 *
 * Apenas uma requisição é enviada por vez. Se novas posições chegam durante o voo,
 * apenas a última é mantida e enviada assim que a anterior terminar. Uma sequência
 * numérica interna descarta respostas stale caso alguma requisição paralela escape.
 */
export function createPositionSaver(save: (mediaId: string, position: number) => Promise<StudyProgress>) {
	let saving = false;
	let queued: { mediaId: string; position: number } | null = null;
	let sequence = 0;
	let applied = 0;

	async function run(mediaId: string, position: number): Promise<SaveResult> {
		sequence += 1;
		const id = sequence;
		saving = true;
		try {
			const progress = await save(mediaId, position);
			if (id < applied) return { kind: 'stale' };
			applied = id;
			return { kind: 'saved', progress };
		} catch (cause) {
			if (id < applied) return { kind: 'stale' };
			return { kind: 'error', message: cause instanceof Error ? cause.message : 'Erro ao salvar progresso.' };
		} finally {
			saving = false;
			if (queued !== null && (queued.mediaId !== mediaId || queued.position !== position)) {
				const next = queued;
				queued = null;
				await run(next.mediaId, next.position);
			}
		}
	}

	return {
		get saving() {
			return saving;
		},
		request(mediaId: string, position: number): Promise<SaveResult> {
			if (saving) {
				queued = { mediaId, position };
				return Promise.resolve({ kind: 'queued' });
			}
			return run(mediaId, position);
		},
		reset() {
			queued = null;
		}
	};
}
