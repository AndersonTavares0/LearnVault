// Append-only guard and lockfile helper for SQLite migrations in crates/core/src/migrations.rs.
//
// Published migrations (V1..Vn) are immutable: never edit an existing migration or rewrite its
// hash in scripts/migrations.lock.json (persistence changes also require independent review per
// .agents/policies/agent-collaboration.md).
//
// When adding a new migration V(n+1) to crates/core/src/migrations.rs, append its lock entry with:
//   node scripts/migrations-append-only.mjs --append
// This helper refuses to run if any previously locked migration hash has changed.

import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
export const DEFAULT_MIGRATIONS_PATH = path.resolve(scriptsDir, '../crates/core/src/migrations.rs');
export const DEFAULT_LOCK_PATH = path.join(scriptsDir, 'migrations.lock.json');

export function sha256(text) {
	return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

export function normalizeNewlines(text) {
	return text.replaceAll('\r\n', '\n');
}

export function readNormalizedMigrations(migrationsPath = DEFAULT_MIGRATIONS_PATH) {
	return normalizeNewlines(fs.readFileSync(migrationsPath, 'utf8'));
}

export function parseMigrations(source) {
	const normalized = normalizeNewlines(source);
	const hashes = {};
	const constRe = /^const (V\d+): &str = "\n([\s\S]*?)^";/gm;
	let match;
	while ((match = constRe.exec(normalized)) !== null) {
		const [, name, sql] = match;
		assert.equal(hashes[name], undefined, `Duplicate migration constant ${name}`);
		hashes[name] = sha256(sql);
	}

	const arrayMatch = normalized.match(
		/pub\(crate\) const MIGRATIONS: &\[&str\] = &\[([\s\S]*?)\];/,
	);
	assert.ok(arrayMatch, 'Could not find MIGRATIONS slice in crates/core/src/migrations.rs');

	const chain = arrayMatch[1]
		.split(',')
		.map((entry) => entry.trim())
		.filter(Boolean);

	assert.ok(chain.length > 0, 'MIGRATIONS slice must not be empty');
	chain.forEach((name, idx) => {
		const expected = `V${idx + 1}`;
		assert.equal(
			name,
			expected,
			`MIGRATIONS[${idx}] must be ${expected} (found ${name}); migrations must be strictly sequential`,
		);
		assert.ok(
			hashes[name],
			`MIGRATIONS[${idx}] references ${name}, but no matching const ${name}: &str was parsed`,
		);
	});

	return { chain, hashes };
}

function assertExistingLocksUnchanged(chain, hashes, lock) {
	const lockKeys = Object.keys(lock);
	assert.ok(lockKeys.length > 0, 'scripts/migrations.lock.json must not be empty');
	assert.ok(
		chain.length >= lockKeys.length,
		`MIGRATIONS has ${chain.length} entries, fewer than locked ${lockKeys.length}; published migrations are immutable`,
	);

	for (let i = 0; i < lockKeys.length; i += 1) {
		const key = lockKeys[i];
		assert.equal(key, `V${i + 1}`, `Lock entry ${i} must be V${i + 1}`);
		assert.equal(
			chain[i],
			key,
			`Migration at index ${i} must remain ${key} (published migrations are immutable)`,
		);
		assert.equal(
			hashes[key],
			lock[key],
			`Published migration ${key} is immutable and cannot be edited! Add a new migration V${chain.length + 1} in crates/core/src/migrations.rs instead, and never rewrite existing hashes in scripts/migrations.lock.json (see .agents/policies/agent-collaboration.md).`,
		);
	}

	return lockKeys;
}

export function verifyAppendOnly(source, lock) {
	const { chain, hashes } = parseMigrations(source);
	const lockKeys = assertExistingLocksUnchanged(chain, hashes, lock);

	assert.equal(
		chain.length,
		lockKeys.length,
		`MIGRATIONS has ${chain.length} entries, but scripts/migrations.lock.json only locks ${lockKeys.length}. Append V${lockKeys.length + 1}..V${chain.length} using: node scripts/migrations-append-only.mjs --append`,
	);
}

export function appendMigrationLockEntries(source, lock) {
	const { chain, hashes } = parseMigrations(source);
	const lockKeys = assertExistingLocksUnchanged(chain, hashes, lock);

	const nextLock = { ...lock };
	for (let i = lockKeys.length; i < chain.length; i += 1) {
		const key = chain[i];
		nextLock[key] = hashes[key];
	}
	return nextLock;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	if (!process.argv.includes('--append')) {
		console.error('Usage: node scripts/migrations-append-only.mjs --append');
		process.exit(1);
	}
	const source = readNormalizedMigrations();
	const lock = JSON.parse(fs.readFileSync(DEFAULT_LOCK_PATH, 'utf8'));
	const nextLock = appendMigrationLockEntries(source, lock);
	const added = Object.keys(nextLock).slice(Object.keys(lock).length);
	if (added.length === 0) {
		console.log('scripts/migrations.lock.json is already up to date.');
	} else {
		fs.writeFileSync(DEFAULT_LOCK_PATH, `${JSON.stringify(nextLock, null, 2)}\n`, 'utf8');
		console.log(`Appended lock entries: ${added.join(', ')}`);
	}
}
