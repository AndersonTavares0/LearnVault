import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import test from 'node:test';

const root = resolve(import.meta.dirname, '..');
const config = JSON.parse(readFileSync(resolve(root, '.github/release-please/release-please-config.json'), 'utf8'));
const release = config.packages['.'];

test('Release Please uses SemVer pre-major bumps, generated names, and categorized notes', () => {
    assert.equal(release['bump-minor-pre-major'], true);
    assert.equal(Object.hasOwn(release, 'bump-patch-for-minor-pre-major'), false);
    assert.equal(release['include-v-in-tag'], true);
    assert.equal(release['include-v-in-release-name'], true);
    assert.equal(release['changelog-path'], 'docs/releases/CHANGELOG.md');
    assert.deepEqual(
        release['changelog-sections'].filter(section => !section.hidden).map(({ type, section }) => [type, section]),
        [
            ['feat', 'Features'],
            ['fix', 'Bug Fixes'],
            ['perf', 'Performance Improvements'],
            ['revert', 'Reverts']
        ]
    );
});
