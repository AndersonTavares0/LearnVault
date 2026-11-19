import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import test from 'node:test';
import { groupCommits, hasMigrationChanges, isPrerelease, parseCommit, parseTag, previousTag, renderChanges, renderMigrations, renderNotes, splitHighlights } from './release-notes.mjs';

const root = resolve(import.meta.dirname, '..');
const read = (path) => readFileSync(resolve(root, path), 'utf8');

test('parses Conventional Commits with scope, pull request number and breaking markers', () => {
	assert.deepEqual(parseCommit('feat(web): show Sign in as one centered form (#593)'), {
		type: 'feat', scope: 'web', subject: 'show Sign in as one centered form', pr: '593', breaking: false, breakingNote: null
	});
	assert.equal(parseCommit('fix: read the digest').scope, null);
	assert.equal(parseCommit('feat(deploy)!: use the published image').breaking, true);
	const footer = parseCommit('feat(deploy): use the image', 'BREAKING CHANGE: Compose pulls the published image.\n\nMore detail.');
	assert.equal(footer.breaking, true);
	assert.equal(footer.breakingNote, 'Compose pulls the published image.');
	assert.equal(parseCommit('Merge branch main'), null);
});

test('keeps only user-visible commit types and links pull requests', () => {
	const commits = [
		parseCommit('feat(branding): add the portico (#599)'),
		parseCommit('docs: rewrite the README (#599)'),
		parseCommit('chore: release 0.9.3 (#598)'),
		parseCommit('fix(server): scan outside the lock (#586)'),
		parseCommit('ci(flatpak): pass the X authority (#579)')
	];
	const grouped = groupCommits(commits);
	assert.deepEqual(grouped.groups.map((group) => group.title), ['Features', 'Bug Fixes']);
	assert.match(grouped.groups[0].lines[0], /^- \*\*branding:\*\* add the portico \(\[#599\]\(https:\/\/github\.com\/AndersonTavares0\/LearnVault\/pull\/599\)\)$/);
	assert.equal(renderChanges({ breaking: [], groups: [] }), 'No user-visible changes.');
});

test('lists breaking changes first', () => {
	const grouped = groupCommits([parseCommit('feat(deploy)!: use the published image (#403)')]);
	assert.match(renderChanges(grouped), /^### Breaking changes\n\n- use the published image/);
});

test('chooses the highest earlier strict release tag and ignores candidates', () => {
	const tags = ['v0.8.0', 'v0.9.0-rc.5', 'v0.9.1', 'v0.9.2', 'v0.10.0', 'nightly'];
	assert.equal(previousTag('v0.9.2', tags), 'v0.9.1');
	assert.equal(previousTag('v0.10.0', tags), 'v0.9.2');
	assert.equal(previousTag('v0.8.0', tags), null);
	assert.throws(() => parseTag('v0.9.3.1'), /strict/);
	assert.throws(() => parseTag('v0.9.0-rc.1'), /strict/);
});

test('splits a highlights file into a summary and a list', () => {
	assert.deepEqual(splitHighlights('A polish release.\n\n- **One:** first.\n- **Two:** second.\n'), {
		summary: 'A polish release.', highlights: '- **One:** first.\n- **Two:** second.'
	});
	assert.deepEqual(splitHighlights('- only a list'), { summary: '', highlights: '- only a list' });
});

test('always states whether the release changes the database', () => {
	assert.equal(renderMigrations(false), 'No database migrations.');
	assert.match(renderMigrations(true), /Back up the data directory/);
});

test('migration notes ignore Rust comments and detect newly appended SQL migrations', () => {
	const source = read('crates/core/src/migrations.rs');
	const commentOnly = source.replace('//! Single migration chain of the SQLite database.', '//! Single, ordered migration chain for the SQLite database.');
	assert.equal(hasMigrationChanges(source, commentOnly), false);
	assert.equal(renderMigrations(hasMigrationChanges(source, commentOnly)), 'No database migrations.');

	const withNewMigration = source.replace(
		'pub(crate) const MIGRATIONS: &[&str] = &[',
		'const V22: &str = "\nCREATE TABLE migration_smoke (id INTEGER PRIMARY KEY);\n";\npub(crate) const MIGRATIONS: &[&str] = &['
	).replace('V19, V20, V21,', 'V19, V20, V21, V22,');
	assert.equal(hasMigrationChanges(source, withNewMigration), true);
	assert.match(renderMigrations(hasMigrationChanges(source, withNewMigration)), /changes the schema migrations/);
});

test('drops the Highlights heading when no highlights were written', () => {
	const template = read('docs/releases/release-notes.template.md');
	const base = { version: '0.9.4', tag: 'v0.9.4', prerelease: ' (pre-release)', changes: '### Features\n\n- one', migrations: 'No database migrations.', compare_url: 'https://example.test/compare' };
	const without = renderNotes(template, { ...base, summary: '', highlights: '' });
	assert.doesNotMatch(without, /## Highlights/);
	assert.match(without, /^# LearnVault 0\.9\.4 \(pre-release\)\n\n## Changes/);
	const withHighlights = renderNotes(template, { ...base, summary: 'A summary.', highlights: '- **One:** first.' });
	assert.match(withHighlights, /A summary\.\n\n## Highlights\n\n- \*\*One:\*\* first\.\n\n## Changes/);
	assert.doesNotMatch(withHighlights, /\{\{/);
});

test('the template lists the asset names the release workflow publishes', () => {
	const template = read('docs/releases/release-notes.template.md');
	const workflow = read('.github/workflows/release.yml');
	for (const name of ['learnvault-linux-amd64.tar.gz', 'learnvault-linux-arm64.tar.gz', 'learnvault-linux-desktop-amd64.flatpak', 'learnvault-linux-desktop-arm64.flatpak', 'latest.json', 'SHA256SUMS']) {
		assert.ok(template.includes(name), `template lists ${name}`);
		assert.ok(workflow.includes(name), `release workflow publishes ${name}`);
	}
	assert.ok(template.includes('learnvault-{{tag}}.sbom.cdx.json'));
	assert.ok(workflow.includes('learnvault-${{ needs.validate-tag.outputs.tag }}.sbom.cdx.json'));
	assert.ok(template.includes('SHA256SUMS.sigstore.json'));
	assert.ok(workflow.includes('dist/SHA256SUMS.sigstore.json'));
	assert.ok(workflow.includes('learnvault-windows-$env:RELEASE_TAG.exe'));
	assert.ok(template.includes('learnvault-windows-{{tag}}.exe') && template.includes('learnvault-windows-{{tag}}.nsis.zip'));
});

test('marks the notes as a pre-release only when the Release Please configuration does', () => {
	assert.equal(isPrerelease({ packages: { '.': { prerelease: true } } }), true);
	assert.equal(isPrerelease({ packages: { '.': { prerelease: false } } }), false);
	assert.equal(isPrerelease({ packages: { '.': {} } }), false);
	const config = JSON.parse(read('.github/release-please/release-please-config.json'));
	assert.equal(isPrerelease(config), false);
});
