import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptsDir, '..');
const migrationsPath = path.join(root, 'crates/core/src/migrations.rs');
const lockPath = path.join(scriptsDir, 'migrations.lock.json');

function sha256(text) {
	return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

function parseMigrations(source) {
	const normalized = source.replaceAll('\r\n', '\n');
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

function verifyAppendOnly(source, lock) {
	const { chain, hashes } = parseMigrations(source);
	const lockKeys = Object.keys(lock);
	assert.ok(lockKeys.length > 0, 'scripts/migrations.lock.json must not be empty');
	assert.ok(
		chain.length >= lockKeys.length,
		`MIGRATIONS has ${chain.length} entries, fewer than locked ${lockKeys.length}`,
	);

	for (let i = 0; i < lockKeys.length; i += 1) {
		const key = lockKeys[i];
		assert.equal(key, `V${i + 1}`, `Lock entry ${i} must be V${i + 1}`);
		assert.equal(
			chain[i],
			key,
			`Migration at index ${i} must remain ${key} (append-only policy)`,
		);
		assert.equal(
			hashes[key],
			lock[key],
			`Published migration ${key} was modified! Migrations V1..V${lockKeys.length} are append-only; add V${chain.length + 1} instead.`,
		);
	}
}

test('published migrations V1..V21 match frozen SHA-256 lock and are append-only', () => {
	const source = fs.readFileSync(migrationsPath, 'utf8');
	const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
	verifyAppendOnly(source, lock);
});

test('editing a published migration fails the append-only guard', () => {
	const source = fs.readFileSync(migrationsPath, 'utf8');
	const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
	const mutated = source.replace('const V18: &str = "\n', 'const V18: &str = "\n-- edited\n');
	assert.notEqual(mutated, source);
	assert.throws(() => verifyAppendOnly(mutated, lock), /Published migration V18 was modified/);
});

test('appending a new migration passes the append-only guard', () => {
	const source = fs.readFileSync(migrationsPath, 'utf8');
	const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
	const appended = source
		.replace(
			'pub(crate) const MIGRATIONS: &[&str] = &[',
			'const V22: &str = "\nSELECT 1;\n";\n\npub(crate) const MIGRATIONS: &[&str] = &[',
		)
		.replace('V21,\n];', 'V21, V22,\n];');
	assert.notEqual(appended, source);
	verifyAppendOnly(appended, lock);
});
