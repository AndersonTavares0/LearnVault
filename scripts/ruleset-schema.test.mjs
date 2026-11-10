import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptsDir, '..');

const learnvaultPath = path.join(root, '.github/rulesets/learnvault-main.json');
const templatePath = path.join(root, '.github/rulesets/default-branch.template.json');

test('learnvault-main.json is valid and enforces required invariants', () => {
	assert.ok(fs.existsSync(learnvaultPath), 'learnvault-main.json must exist');
	const source = fs.readFileSync(learnvaultPath, 'utf8');
	assert.equal(source.match(/"allowed_merge_methods"\s*:/g)?.length, 1, 'JSON must not contain duplicate merge policy keys');
	const content = JSON.parse(source);

	assert.equal(content.target, 'branch');
	assert.equal(content.enforcement, 'active');
	assert.deepEqual(content.bypass_actors, []);
	assert.deepEqual(content.conditions?.ref_name?.include, ['~DEFAULT_BRANCH']);

	const ruleTypes = content.rules.map((r) => r.type);
	assert.ok(ruleTypes.includes('deletion'), 'Must prevent branch deletion');
	assert.ok(ruleTypes.includes('non_fast_forward'), 'Must prevent force pushes');
	assert.ok(ruleTypes.includes('pull_request'), 'Must require pull request');
	assert.ok(ruleTypes.includes('required_status_checks'), 'Must require status checks');

	const prRule = content.rules.find((r) => r.type === 'pull_request');
	assert.equal(prRule.parameters.required_approving_review_count, 0, 'Solo maintainer compatibility');
	assert.deepEqual(prRule.parameters.allowed_merge_methods, ['squash'], 'Production merges follow the squash-only policy');
	assert.equal(prRule.parameters.required_review_thread_resolution, true, 'Conversation resolution mandatory');
	assert.equal(prRule.parameters.dismiss_stale_reviews_on_push, true, 'Dismiss stale reviews on push');

	const checksRule = content.rules.find((r) => r.type === 'required_status_checks');
	assert.equal(checksRule.parameters.strict_required_status_checks_policy, true, 'Strict policy required');
	const contexts = checksRule.parameters.required_status_checks.map((c) => c.context);
	assert.deepEqual(
		contexts.sort(),
		[
			'CI gate',
			'Container build (Rust)',
			'Desktop crate compile and guards',
			'Rust tests and lints',
			'SPA checks'
		].sort()
	);
});

test('default-branch.template.json is valid and portable', () => {
	assert.ok(fs.existsSync(templatePath), 'default-branch.template.json must exist');
	const content = JSON.parse(fs.readFileSync(templatePath, 'utf8'));

	assert.equal(content.target, 'branch');
	assert.equal(content.enforcement, 'active');
	assert.deepEqual(content.bypass_actors, []);
	assert.deepEqual(content.conditions?.ref_name?.include, ['~DEFAULT_BRANCH']);

	const ruleTypes = content.rules.map((r) => r.type);
	assert.ok(ruleTypes.includes('deletion'));
	assert.ok(ruleTypes.includes('non_fast_forward'));
	assert.ok(ruleTypes.includes('pull_request'));
	assert.ok(ruleTypes.includes('required_status_checks'));

	const prRule = content.rules.find((r) => r.type === 'pull_request');
	assert.equal(prRule.parameters.required_approving_review_count, 0);
	assert.equal(prRule.parameters.required_review_thread_resolution, true);

	const checksRule = content.rules.find((r) => r.type === 'required_status_checks');
	assert.equal(checksRule.parameters.strict_required_status_checks_policy, true);
	assert.deepEqual(checksRule.parameters.required_status_checks, [], 'Generic template must not hardcode repo checks');
});
