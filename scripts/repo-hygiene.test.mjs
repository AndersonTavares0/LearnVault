import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { checkTrackedFiles, PROHIBITED_PATH_PREFIXES, ALLOWED_IMAGE_DIRS } from './repo-hygiene.mjs';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptsDir, '..');

test('repository contains zero prohibited paths, unapproved raster images, or bloated files', () => {
	const output = execFileSync('git', ['ls-files'], { cwd: root, encoding: 'utf-8' });
	const files = output.trim().split('\n').filter(Boolean);

	const violations = checkTrackedFiles(files, { root });
	assert.deepEqual(violations, [], `Hygiene violations found in tracked repository files:\n${JSON.stringify(violations, null, 2)}`);
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
		'docs/qa/test.webp'
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
		'apps/web/public/favicon.ico',
		'apps/web/public/apple-touch-icon.png',
		'testdata/library/sample.mp4'
	];
	assert.deepEqual(checkTrackedFiles(permitted, { root }), []);

	const forbidden = [
		'README-screenshot.png',
		'docs/architecture/diagram.png',
		'apps/web/src/assets/hero.jpg',
		'docs/design/icon.ico',
		'docs/guide/demo.mkv'
	];
	const violations = checkTrackedFiles(forbidden, { root });
	assert.equal(violations.length, 5);
});

test('hygiene checker normalizes Windows backslash paths', () => {
	const syntheticWindowsFiles = [
		'.claude\\settings.json',
		'docs\\design\\screenshots\\shot.png',
		'apps\\web\\public\\favicon.ico'
	];
	const violations = checkTrackedFiles(syntheticWindowsFiles, { root });
	assert.equal(violations.length, 2);
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
