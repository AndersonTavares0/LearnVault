import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import test from 'node:test';

const root = resolve(import.meta.dirname, '..');
const config = JSON.parse(readFileSync(resolve(root, '.github/release-please/release-please-config.json'), 'utf8'));
const release = config.packages['.'];
const workflow = readFileSync(resolve(root, '.github/workflows/release-please.yml'), 'utf8');

test('Release Please uses SemVer pre-major bumps, generated names, and categorized notes', () => {
    assert.equal(release['bump-minor-pre-major'], true);
    assert.equal(release['bump-patch-for-minor-pre-major'], true);
    assert.equal(release.versioning, 'default');
    assert.equal(release.prerelease, false);
    assert.equal(release['prerelease-type'], undefined);
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

test('Release Please prepares releases monthly and publishes merged release PRs immediately', () => {
    assert.match(workflow, /schedule:\n\s+- cron: '0 12 1 \* \*'/);
    assert.match(workflow, /^  workflow_dispatch:$/m);
    assert.match(workflow, /push:\n\s+branches: \[main\]/);
    assert.match(workflow, /skip-github-release: \$\{\{ github\.event_name != 'push' \}\}/);
    assert.match(workflow, /skip-github-pull-request: \$\{\{ github\.event_name == 'push' \}\}/);
});
