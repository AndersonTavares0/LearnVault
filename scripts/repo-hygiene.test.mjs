import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { checkTrackedFiles, PROHIBITED_PATH_PREFIXES, ALLOWED_IMAGE_DIRS } from './repo-hygiene.mjs';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptsDir, '..');

test('tracked repository files pass hygiene checks', () => {
	const output = execFileSync('git', ['ls-files', '-z'], { cwd: root, encoding: 'utf-8' });
	const files = output.split('\0').filter(Boolean);

	const violations = checkTrackedFiles(files, { root });
	const locations = violations.map(({ file, line, rule }) =>
		`${file}${line === undefined ? '' : `:${line}`} ${rule}`
	);
	assert.ok(locations.length === 0, `Repository hygiene violations:\n${locations.join('\n')}`);
});

test('hygiene checker catches prohibited directory prefixes', () => {
	const syntheticFiles = [
		'.impeccable/design.json',
		'.claude/settings.json',
		'.gemini/state.json',
		'.cursor/rules.json',
		'.windsurf/config.json',
		'.copilot/prompt.md',
		'.github/tdd/scratch.md',
		'docs/design/screenshots/shot.png',
		'docs/design/prototypes/test.html',
		'docs/qa/test.webp',
		'.pnpm-store/v11/index.db',
		'node_modules/pkg/index.js'
	];

	const violations = checkTrackedFiles(syntheticFiles, { root });
	assert.equal(violations.length, syntheticFiles.length);
	for (const prefix of PROHIBITED_PATH_PREFIXES) {
		assert.ok(violations.some((v) => v.file.startsWith(prefix)));
	}
});

test('hygiene checker permits raster images and fixtures strictly in allowed directories', () => {
	const permitted = [
		'crates/desktop/icons/icon.png',
		'crates/desktop/icons/icon.ico',
		'crates/desktop/icons/128x128.png',
		'deploy/packages/app.learnvault.desktop.png',
		'testdata/library/sample.mp4'
	];
	assert.deepEqual(checkTrackedFiles(permitted, { root }), []);

	const forbidden = [
		'README-screenshot.png',
		'docs/architecture/diagram.png',
		'docs/design/icon.ico',
		'docs/guide/demo.mkv'
	];
	const violations = checkTrackedFiles(forbidden, { root });
	assert.equal(violations.length, 4);
});

test('hygiene checker normalizes Windows backslash paths', () => {
	const syntheticWindowsFiles = [
		'.claude\\settings.json',
		'docs\\design\\screenshots\\shot.png',
		'apps\\web\\src\\hero.ico'
	];
	const violations = checkTrackedFiles(syntheticWindowsFiles, { root });
	assert.equal(violations.length, 3);
	assert.ok(violations.some((v) => v.file.includes('.claude')));
	assert.ok(violations.some((v) => v.file.includes('screenshots')));
});

test('hygiene checker enforces vector-only branding assets', () => {
	const valid = ['docs/design/branding/learnvault-banner.svg', 'docs/design/branding/README.md'];
	assert.deepEqual(checkTrackedFiles(valid, { root }), []);

	const invalid = ['docs/design/branding/banner.png', 'docs/design/branding/logo.webp'];
	const violations = checkTrackedFiles(invalid, { root });
	assert.equal(violations.length, 2);
});

test('hygiene checker blocks tracked credentials, runtime data, and local files', () => {
	const forbidden = [
		'deploy/.env', 'deploy/.env.production', 'deploy/.env.example.bak',
		'deploy/compose/secrets/password.txt', 'cert.PEM', 'signing.key', 'signing.key.pub',
		'cert.p12', 'cert.pfx', 'cert.jks', 'cert.keystore', 'ssh/id_rsa', 'ssh/id_rsa.pub',
		'data.sqlite', 'data.sqlite3', 'data.sqlite-journal', 'data.db', 'data.db-wal',
		'data.db-shm', 'runtime.log', 'docs/COORDINATION.local.md', 'notes.local.md'
	];
	assert.deepEqual(checkTrackedFiles(forbidden, { root }).map(({ file }) => file), forbidden);
	assert.deepEqual(checkTrackedFiles(['.env.example', 'deploy/.env.dev.example'], { root }), []);
});

test('hygiene checker reports token locations without exposing their values', (t) => {
	const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'learnvault-hygiene-'));
	t.after(() => fs.rmSync(fixtureRoot, { recursive: true, force: true }));
	const fixtures = [
		['AWS access key ID', 'AKIA' + 'Z'.repeat(16)],
		...['p', 'o', 'u', 's', 'r'].map((type) => ['GitHub token', 'gh' + type + '_' + 'Z'.repeat(36)]),
		['GitHub token', 'github_pat_' + 'Z'.repeat(22) + '_' + 'Z'.repeat(59)],
		['PEM private key header', '-----BEGIN OPENSSH PRIVATE KEY-----'],
		['Slack token', 'xoxb-' + '0'.repeat(12) + '-' + 'Z'.repeat(24)],
		['Google API key', 'AIza' + 'Z'.repeat(35)],
		['Long sk- key', 'sk-proj-' + 'Z'.repeat(40)],
		['JWT', 'eyJ' + 'Z'.repeat(12) + '.eyJ' + 'Z'.repeat(8) + '.' + 'Z'.repeat(32)]
	];
	const file = 'nested-config';
	fs.writeFileSync(path.join(fixtureRoot, file), ['# Fixture', ...fixtures.map(([, value]) => value)].join('\r\n'));
	const violations = checkTrackedFiles([file], { root: fixtureRoot });
	assert.deepEqual(violations, fixtures.map(([rule], index) => ({ file, line: index + 2, rule })));
});

test('fixture exemption applies only to the specified path and pattern', (t) => {
	const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'learnvault-hygiene-'));
	t.after(() => fs.rmSync(fixtureRoot, { recursive: true, force: true }));
	const file = 'scripts/repo-hygiene.test.mjs';
	fs.mkdirSync(path.join(fixtureRoot, 'scripts'));
	const header = '-----BEGIN OPENSSH ' + 'PRIVATE KEY-----';
	const token = 'ghp_' + 'Z'.repeat(36);
	const fixtureLine = "\t\t['PEM private key header', '" + header + "'],";
	fs.writeFileSync(path.join(fixtureRoot, file), [fixtureLine, header, token].join('\n'));
	fs.writeFileSync(path.join(fixtureRoot, 'other.txt'), fixtureLine);
	assert.deepEqual(checkTrackedFiles([file, 'other.txt'], { root: fixtureRoot }), [
		{ file, line: 2, rule: 'PEM private key header' },
		{ file, line: 3, rule: 'GitHub token' },
		{ file: 'other.txt', line: 1, rule: 'PEM private key header' }
	]);
});
