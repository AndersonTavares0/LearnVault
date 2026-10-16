// Regressão do gate de segurança do workflow `Dependabot safe auto-merge`.
//
// O gate de alerta deve falhar fechado: só uma PR é elegível quando o estado
// de alerta é explicitamente `null`. Estados ausentes, vazios ou desconhecidos
// precisam desqualificar a PR. Este teste trava a regressão em que um estado
// vazio era tratado como "sem alerta" (fail-open) no passo `eligibility`.
//
// Execução: `pnpm test:scripts` (ou `node --test scripts/`).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const workflowPath = path.join(root, '.github', 'workflows', 'dependabot-automerge.yml');

// Recorta o bloco shell do passo `eligibility` para inspecionar a lógica exata
// do gate de alerta sem depender de um parser YAML completo.
function eligibilityScript() {
	const source = readFileSync(workflowPath, 'utf8');
	const start = source.indexOf('id: eligibility');
	assert.notEqual(start, -1, 'passo eligibility ausente no workflow');
	const end = source.indexOf('Confirm clean current pull request', start);
	assert.notEqual(end, -1, 'fim do passo eligibility não localizado');
	return source.slice(start, end);
}

test('o gate de alerta exige o valor explícito null', () => {
	const script = eligibilityScript();
	assert.match(
		script,
		/\[\s*"\$ALERT_STATE"\s*!=\s*"null"\s*\]/,
		'o gate deve comparar ALERT_STATE com "null" de forma explícita'
	);
});

test('o gate de alerta desqualifica a PR quando o estado não é null', () => {
	const script = eligibilityScript();
	// A comparação estrita precisa zerar eligible; caso contrário o gate seria
	// decorativo e uma PR seguiria elegível mesmo com alerta/estado inválido.
	const guard = script.match(/if\s*\[\s*"\$ALERT_STATE"\s*!=\s*"null"\s*\]\s*;?\s*then([\s\S]*?)fi/);
	assert.ok(guard, 'bloco `if [ "$ALERT_STATE" != "null" ]` não encontrado');
	assert.match(guard[1], /eligible=false/, 'o gate deve definir eligible=false');
});

test('o gate de alerta falha fechado para estado vazio ou desconhecido', () => {
	const script = eligibilityScript();
	// O padrão antigo `[ -n "$ALERT_STATE" ] && [ ... != null ]` tratava um
	// estado vazio como seguro (fail-open); ele não pode voltar.
	assert.doesNotMatch(
		script,
		/-n\s+"\$ALERT_STATE"/,
		'gate voltou a tolerar ALERT_STATE vazio (fail-open)'
	);
	// Nenhum curto-circuito de "vazio é seguro" pode preceder a comparação estrita.
	assert.doesNotMatch(
		script,
		/\$ALERT_STATE"\s*\]\s*\|\|\s*eligible=true/,
		'ALERT_STATE vazio não pode tornar a PR elegível'
	);
});
