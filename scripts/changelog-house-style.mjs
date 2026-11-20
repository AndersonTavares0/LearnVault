#!/usr/bin/env node
// Rewrites the newest docs/releases/CHANGELOG.md section into the house style used since v0.9.5:
// a summary paragraph and a "### Highlights" list (from docs/releases/highlights/<version>.md) above the generated lists.
// Idempotent, so it can run on every update of the Release Please pull request.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { MIGRATIONS_FILE, hasMigrationChanges, previousTag, splitHighlights } from './release-notes.mjs';

export function renderDatabaseItem(changed) {
	return changed
		? `- **Database:** this release changes the schema migrations (\`${MIGRATIONS_FILE}\`). They run automatically on startup. Back up the data directory before upgrading.`
		: '- **Database:** no schema migrations in this release.';
}

/** Inserts summary and Highlights under the heading of `version`; returns the text unchanged when already styled or not applicable. */
export function applyHouseStyle(changelog, version, { summary, highlights, migrationsChanged }) {
	const text = changelog.replace(/\r\n/g, '\n');
	const heading = new RegExp(`^## \\[${version.replaceAll('.', '\\.')}\\][^\\n]*\\n`, 'm').exec(text);
	if (!heading || !highlights) return text;
	const start = heading.index + heading[0].length;
	const next = text.indexOf('\n## [', start);
	const section = text.slice(start, next === -1 ? text.length : next);
	if (/^### Highlights$/m.test(section)) return text;
	const block = [summary ? `${summary}\n\n` : '', '### Highlights\n\n', `${highlights}\n`, `${renderDatabaseItem(migrationsChanged)}\n`].join('');
	return `${text.slice(0, start)}\n${block}\n${text.slice(start).replace(/^\n+/, '\n')}`;
}

const git = (...args) => execFileSync('git', args, { encoding: 'utf8' });

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const root = process.cwd();
	const version = process.argv[2] ?? readFileSync(resolve(root, '.github/release-please/version.txt'), 'utf8').trim();
	const file = resolve(root, 'docs/releases/CHANGELOG.md');
	const highlightsPath = resolve(root, 'docs/releases/highlights', `${version}.md`);
	if (!existsSync(highlightsPath)) {
		console.log(`::warning::docs/releases/highlights/${version}.md is missing; the changelog keeps only the generated lists.`);
		process.exit(0);
	}
	const tags = git('tag', '--list').split('\n').filter(Boolean);
	const base = previousTag(`v${version}`, tags);
	const migrationsChanged = base ? hasMigrationChanges(git('show', `${base}:${MIGRATIONS_FILE}`), readFileSync(resolve(root, MIGRATIONS_FILE), 'utf8')) : false;
	const { summary, highlights } = splitHighlights(readFileSync(highlightsPath, 'utf8'));
	const before = readFileSync(file, 'utf8');
	const after = applyHouseStyle(before, version, { summary, highlights, migrationsChanged });
	if (after !== before) writeFileSync(file, after);
	console.log(after === before ? 'Changelog already in house style.' : `Changelog ${version} rewritten in house style.`);
}
