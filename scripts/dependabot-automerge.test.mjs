// Regression test for the security gate of the `Dependabot safe auto-merge` workflow.
//
// The alert gate must fail closed: a PR is only eligible when the alert state
// is explicitly `null`. Missing, empty or unknown states
// must disqualify the PR. This test locks in the regression where an empty state
// was treated as "no alert" (fail-open) in the `eligibility` step.
//
// Run: `pnpm test:scripts` (or `node --test scripts/`).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const workflowPath = path.join(root, '.github', 'workflows', 'dependabot-automerge.yml');

// Cuts out the shell block of the `eligibility` step to inspect the exact logic
// of the alert gate without depending on a full YAML parser.
function eligibilityScript() {
	const source = readFileSync(workflowPath, 'utf8');
	const start = source.indexOf('id: eligibility');
	assert.notEqual(start, -1, 'passo eligibility ausente no workflow');
	const end = source.indexOf('Confirm clean current pull request', start);
	assert.notEqual(end, -1, 'end of the eligibility step not found');
	return source.slice(start, end);
}

test('the alert gate requires the explicit value null', () => {
	const script = eligibilityScript();
	assert.match(
		script,
		/\[\s*"\$ALERT_STATE"\s*!=\s*"null"\s*\]/,
		'the gate must compare ALERT_STATE with "null" explicitly'
	);
});

test('the alert gate disqualifies the PR when the state is not null', () => {
	const script = eligibilityScript();
	// The strict comparison must reset eligible; otherwise the gate would be
	// decorative and a PR would stay eligible even with an alert/invalid state.
	const guard = script.match(/if\s*\[\s*"\$ALERT_STATE"\s*!=\s*"null"\s*\]\s*;?\s*then([\s\S]*?)fi/);
	assert.ok(guard, 'block `if [ "$ALERT_STATE" != "null" ]` not found');
	assert.match(guard[1], /eligible=false/, 'the gate must set eligible=false');
});

test('the alert gate fails closed for an empty or unknown state', () => {
	const script = eligibilityScript();
	// The old pattern `[ -n "$ALERT_STATE" ] && [ ... != null ]` treated an
	// empty state as safe (fail-open); it must not come back.
	assert.doesNotMatch(
		script,
		/-n\s+"\$ALERT_STATE"/,
		'gate voltou a tolerar ALERT_STATE vazio (fail-open)'
	);
	// No "empty is safe" short circuit may precede the strict comparison.
	assert.doesNotMatch(
		script,
		/\$ALERT_STATE"\s*\]\s*\|\|\s*eligible=true/,
		'an empty ALERT_STATE cannot make the PR eligible'
	);
});
