import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

import {
	DEFAULT_LOCK_PATH,
	DEFAULT_MIGRATIONS_PATH,
	appendMigrationLockEntries,
	parseMigrations,
	readNormalizedMigrations,
	verifyAppendOnly,
} from './migrations-append-only.mjs';

function appendSyntheticMigration(source) {
	const { chain } = parseMigrations(source);
	const last = chain[chain.length - 1];
	const next = `V${chain.length + 1}`;

	const withConst = source.replace(
		'pub(crate) const MIGRATIONS: &[&str] = &[',
		`const ${next}: &str = "\nSELECT 1;\n";\n\npub(crate) const MIGRATIONS: &[&str] = &[`,
	);
	assert.notEqual(withConst, source, 'Expected to insert synthetic migration constant');

	const arrayRe = new RegExp(`(${last},?\\s*\\n\\];)`);
	const withArrayEntry = withConst.replace(arrayRe, `${last}, ${next},\n];`);
	assert.notEqual(withArrayEntry, withConst, `Expected to append ${next} after ${last} in MIGRATIONS`);
	return { appendedSource: withArrayEntry, next };
}

test('published migrations match frozen SHA-256 lock and are immutable', () => {
	const source = readNormalizedMigrations(DEFAULT_MIGRATIONS_PATH);
	const lock = JSON.parse(fs.readFileSync(DEFAULT_LOCK_PATH, 'utf8'));
	verifyAppendOnly(source, lock);
});

test('editing a published migration fails the immutability guard', () => {
	const source = readNormalizedMigrations(DEFAULT_MIGRATIONS_PATH);
	const lock = JSON.parse(fs.readFileSync(DEFAULT_LOCK_PATH, 'utf8'));
	const mutated = source.replace('const V18: &str = "\n', 'const V18: &str = "\n-- edited\n');
	assert.notEqual(mutated, source);
	assert.throws(
		() => verifyAppendOnly(mutated, lock),
		/Published migration V18 is immutable and cannot be edited! Add a new migration/,
	);
});

test('appending a new migration passes with a lock entry and fails without one', () => {
	const source = readNormalizedMigrations(DEFAULT_MIGRATIONS_PATH);
	const lock = JSON.parse(fs.readFileSync(DEFAULT_LOCK_PATH, 'utf8'));
	const { appendedSource, next } = appendSyntheticMigration(source);

	assert.throws(
		() => verifyAppendOnly(appendedSource, lock),
		new RegExp(`Append ${next}..${next}`),
	);

	const updatedLock = appendMigrationLockEntries(appendedSource, lock);
	assert.ok(updatedLock[next], `Expected ${next} to be added to lock`);
	verifyAppendOnly(appendedSource, updatedLock);
});

test('appendMigrationLockEntries refuses to rewrite existing hashes', () => {
	const source = readNormalizedMigrations(DEFAULT_MIGRATIONS_PATH);
	const lock = JSON.parse(fs.readFileSync(DEFAULT_LOCK_PATH, 'utf8'));
	const mutated = source.replace('const V18: &str = "\n', 'const V18: &str = "\n-- edited\n');
	assert.throws(
		() => appendMigrationLockEntries(mutated, lock),
		/Published migration V18 is immutable/,
	);
});

test('CRLF line endings are handled identically in immutability and append checks', () => {
	const source = readNormalizedMigrations(DEFAULT_MIGRATIONS_PATH);
	const crlfSource = source.replaceAll('\n', '\r\n');
	const lock = JSON.parse(fs.readFileSync(DEFAULT_LOCK_PATH, 'utf8'));

	verifyAppendOnly(crlfSource, lock);

	const mutatedCrlf = crlfSource.replace('const V18: &str = "\r\n', 'const V18: &str = "\r\n-- edited\r\n');
	assert.notEqual(mutatedCrlf, crlfSource);
	assert.throws(
		() => verifyAppendOnly(mutatedCrlf, lock),
		/Published migration V18 is immutable/,
	);

	const { appendedSource } = appendSyntheticMigration(source);
	const appendedCrlf = appendedSource.replaceAll('\n', '\r\n');
	const updatedLock = appendMigrationLockEntries(appendedCrlf, lock);
	verifyAppendOnly(appendedCrlf, updatedLock);
});
