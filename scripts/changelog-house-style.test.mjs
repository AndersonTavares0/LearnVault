import assert from 'node:assert/strict';
import { test } from 'node:test';
import { applyHouseStyle, renderDatabaseItem } from './changelog-house-style.mjs';

const changelog = '# Changelog\n\n## [0.9.6](https://x/compare/v0.9.5...v0.9.6) (2026-11-20)\n\n\n### Bug Fixes\n\n* **a:** fix\n\n## [0.9.5](https://x) (2026-11-19)\n\nOld summary.\n\n### Highlights\n\n- old\n';
const input = { summary: 'A patch release.', highlights: '- **One:** thing.', migrationsChanged: false };

test('inserts summary, highlights and the database line above the generated lists', () => {
	const out = applyHouseStyle(changelog, '0.9.6', input);
	assert.match(out, /\(2026-11-20\)\n\nA patch release\.\n\n### Highlights\n\n- \*\*One:\*\* thing\.\n- \*\*Database:\*\* no schema migrations in this release\.\n\n+### Bug Fixes/);
	assert.ok(out.includes('Old summary.'));
});

test('is idempotent and leaves styled sections alone', () => {
	const once = applyHouseStyle(changelog, '0.9.6', input);
	assert.equal(applyHouseStyle(once, '0.9.6', input), once);
	assert.equal(applyHouseStyle(changelog, '0.9.5', input), changelog);
});

test('does nothing without highlights or for an unknown version', () => {
	assert.equal(applyHouseStyle(changelog, '0.9.6', { ...input, highlights: '' }), changelog);
	assert.equal(applyHouseStyle(changelog, '1.0.0', input), changelog);
});

test('states changed migrations', () => {
	assert.match(renderDatabaseItem(true), /changes the schema migrations/);
});
