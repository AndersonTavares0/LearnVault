// Release notes generator. Fills docs/releases/release-notes.template.md for one release tag.
//
//   node scripts/release-notes.mjs v0.9.4 [--from v0.9.3] [--stable | --pre-release]
//
// The grouped changes come from Conventional Commits between the previous release and the tag (or HEAD while the tag does
// not exist yet). Hand-written highlights are read from docs/releases/highlights/<version>.md when that file exists.
// The notes are printed to stdout; publish them with `gh release edit <tag> --notes-file -`.
// Policy: .agents/policies/releases.md.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseMigrations } from './migrations-append-only.mjs';

export const REPOSITORY_URL = 'https://github.com/AndersonTavares0/LearnVault';
export const MIGRATIONS_FILE = 'crates/core/src/migrations.rs';
const STRICT_TAG = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const SECTIONS = [
	['feat', 'Features'],
	['fix', 'Bug Fixes'],
	['perf', 'Performance Improvements'],
	['revert', 'Reverts']
];

export function parseTag(tag) {
	const match = STRICT_TAG.exec(tag);
	if (!match) throw new Error(`Not a strict vMAJOR.MINOR.PATCH tag: ${tag}`);
	return match.slice(1).map(Number);
}

const compareTags = (a, b) => {
	const [pa, pb] = [parseTag(a), parseTag(b)];
	for (let i = 0; i < 3; i += 1) if (pa[i] !== pb[i]) return pa[i] - pb[i];
	return 0;
};

/** The highest strict release tag below `tag`. Release candidates and other tags are ignored. */
export function previousTag(tag, tags) {
	const lower = tags.filter((candidate) => STRICT_TAG.test(candidate) && compareTags(candidate, tag) < 0);
	return lower.sort(compareTags).at(-1) ?? null;
}

/** Parses `type(scope)!: subject (#123)` plus an optional body. Returns null for non-conventional subjects. */
export function parseCommit(subject, body = '') {
	const match = /^(\w+)(?:\(([^)]+)\))?(!)?: (.+)$/.exec(subject.trim());
	if (!match) return null;
	const [, type, scope, bang, rest] = match;
	const pr = /\(#(\d+)\)\s*$/.exec(rest)?.[1] ?? null;
	const text = rest.replace(/\s*\(#\d+\)\s*$/, '').trim();
	const footer = /^BREAKING[ -]CHANGE:\s*(.+)$/ms.exec(body)?.[1]?.split('\n\n')[0].trim();
	return { type, scope: scope ?? null, subject: text, pr, breaking: Boolean(bang) || Boolean(footer), breakingNote: footer ?? null };
}

const line = (commit) => {
	const scope = commit.scope ? `**${commit.scope}:** ` : '';
	const pr = commit.pr ? ` ([#${commit.pr}](${REPOSITORY_URL}/issues/${commit.pr}))` : '';
	return `- ${scope}${commit.subject}${pr}`;
};

/** Groups user-visible commits. chore, docs, ci, test, build, style and refactor stay out of the notes. */
export function groupCommits(commits) {
	const groups = SECTIONS.map(([type, title]) => ({ title, lines: commits.filter((commit) => commit.type === type).map(line) }));
	const breaking = commits.filter((commit) => commit.breaking).map((commit) => `- ${commit.breakingNote ?? commit.subject}${commit.pr ? ` ([#${commit.pr}](${REPOSITORY_URL}/issues/${commit.pr}))` : ''}`);
	return { breaking, groups: groups.filter((group) => group.lines.length > 0) };
}

export function renderChanges({ breaking, groups }) {
	const parts = [];
	if (breaking.length > 0) parts.push(`### Breaking changes\n\n${breaking.join('\n')}`);
	for (const group of groups) parts.push(`### ${group.title}\n\n${group.lines.join('\n')}`);
	return parts.length > 0 ? parts.join('\n\n') : 'No user-visible changes.';
}

/** A highlights file starts with a summary paragraph, then the list. Both parts are optional. */
export function splitHighlights(text) {
	const lines = text.replace(/\r\n/g, '\n').trim().split('\n');
	const firstItem = lines.findIndex((entry) => /^[-*] /.test(entry));
	if (firstItem === -1) return { summary: lines.join('\n').trim(), highlights: '' };
	return { summary: lines.slice(0, firstItem).join('\n').trim(), highlights: lines.slice(firstItem).join('\n').trim() };
}

export function renderMigrations(changed) {
	return changed
		? `**Database:** this release changes the schema migrations (\`${MIGRATIONS_FILE}\`). They run automatically on startup. Back up the data directory before upgrading.`
		: 'No database migrations.';
}

export function hasMigrationChanges(baseSource, headSource) {
	const base = parseMigrations(baseSource).hashes;
	const head = parseMigrations(headSource).hashes;
	return Object.keys(base).length !== Object.keys(head).length
		|| Object.keys(head).some((name) => base[name] !== head[name]);
}

/** Fills the template. A section whose value is empty is removed together with its heading. */
export function renderNotes(template, values) {
	let output = template;
	for (const [name, value] of Object.entries(values)) output = output.replaceAll(`{{${name}}}`, value);
	output = output.replace(/^## Highlights\n\n(?=##|\n|$)/m, '').replace(/\n{3,}/g, '\n\n');
	return `${output.trim()}\n`;
}

const git = (...args) => execFileSync('git', args, { encoding: 'utf8' });

function readCommits(range) {
	const raw = git('log', '--no-merges', '--format=%s%x1f%b%x1e', range);
	return raw.split('\x1e').map((entry) => entry.trim()).filter(Boolean).map((entry) => {
		const [subject, body = ''] = entry.split('\x1f');
		return parseCommit(subject, body);
	}).filter(Boolean);
}

/** Whether releases are published as pre-releases, from the Release Please configuration. */
export function isPrerelease(config) {
	return config.packages?.['.']?.prerelease === true;
}

export function generate({ tag, from, stable, root = process.cwd() }) {
	parseTag(tag);
	const tags = git('tag', '--list').split('\n').filter(Boolean);
	const base = from ?? previousTag(tag, tags);
	if (!base) throw new Error(`No earlier release tag found before ${tag}; pass --from.`);
	const head = tags.includes(tag) ? tag : 'HEAD';
	const version = tag.slice(1);
	const baseMigrations = git('show', `${base}:${MIGRATIONS_FILE}`);
	const headMigrations = git('show', `${head}:${MIGRATIONS_FILE}`);
	const migrationsChanged = hasMigrationChanges(baseMigrations, headMigrations);
	const highlightsPath = resolve(root, 'docs/releases/highlights', `${version}.md`);
	const { summary, highlights } = existsSync(highlightsPath) ? splitHighlights(readFileSync(highlightsPath, 'utf8')) : { summary: '', highlights: '' };
	const template = readFileSync(resolve(root, 'docs/releases/release-notes.template.md'), 'utf8');
	return renderNotes(template, {
		version,
		tag,
		prerelease: (stable === undefined ? isPrerelease(JSON.parse(readFileSync(resolve(root, '.github/release-please/release-please-config.json'), 'utf8'))) : !stable) ? ' (pre-release)' : '',
		summary,
		highlights,
		changes: renderChanges(groupCommits(readCommits(`${base}..${head}`))),
		migrations: renderMigrations(migrationsChanged),
		compare_url: `${REPOSITORY_URL}/compare/${base}...${tag}`
	});
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const args = process.argv.slice(2);
	const tag = args.find((arg) => !arg.startsWith('--'));
	const fromIndex = args.indexOf('--from');
	if (!tag) {
		console.error('Usage: node scripts/release-notes.mjs vX.Y.Z [--from vX.Y.Z] [--stable | --pre-release]');
		process.exit(2);
	}
	try {
		process.stdout.write(generate({ tag, from: fromIndex === -1 ? undefined : args[fromIndex + 1], stable: args.includes('--stable') ? true : args.includes('--pre-release') ? false : undefined }));
	} catch (error) {
		console.error(error.message);
		process.exit(1);
	}
}
