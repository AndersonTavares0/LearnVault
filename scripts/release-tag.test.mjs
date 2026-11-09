import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const root = resolve(import.meta.dirname, '..');
const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
function runBlock(name) {
    const step = workflow.split(`      - name: ${name}\n`)[1];
    assert.ok(step, `Missing step: ${name}`);
    const lines = step.split('        run: |\n')[1].split('\n');
    const end = lines.findIndex(line => line.trim() && !line.startsWith('          '));
    return lines.slice(0, end < 0 ? undefined : end).map(line => line.slice(10)).join('\n');
}
function fixture(t) {
    const directory = mkdtempSync(join(tmpdir(), 'learnvault-release-'));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    const git = (...args) => execFileSync('git', args, { cwd: directory, encoding: 'utf8' }).trim();
    git('init', '--quiet');
    git('-c', 'user.name=Security Test', '-c', 'user.email=test@example.invalid', 'commit', '--quiet', '--allow-empty', '-m', 'Synthetic release');
    const commit = git('rev-parse', 'HEAD');
    git('-c', 'user.name=Security Test', '-c', 'user.email=test@example.invalid', 'tag', '-a', 'v1.2.3', '-m', 'Synthetic tag', '--no-sign');
    return { directory, git, commit };
}

test('release resolution uses the requested tag when dispatch HEAD has advanced', t => {
    const { directory, git, commit } = fixture(t);
    git('-c', 'user.name=Security Test', '-c', 'user.email=test@example.invalid', 'commit', '--quiet', '--allow-empty', '-m', 'Later synthetic change');
    const output = join(directory, 'outputs');
    execFileSync('bash', ['-c', runBlock('Resolve release tag commit')], {
        cwd: directory, env: { PATH: process.env.PATH, RELEASE_TAG: 'v1.2.3', GITHUB_OUTPUT: output }
    });
    assert.equal(readFileSync(output, 'utf8').trim(), `commit_sha=${commit}`);
    assert.match(workflow, /ref: refs\/tags\/\$\{\{ steps\.tag\.outputs\.tag \}\}/);
});

for (const name of ['Confirm release tag did not move', 'Confirm container release tag did not move']) {
    for (const moved of [false, true]) {
        test(`${name}: ${moved ? 'rejects changed' : 'accepts unchanged'} annotated tag`, t => {
            const { directory, git, commit } = fixture(t);
            const remote = join(directory, 'remote.git');
            git('clone', '--quiet', '--bare', directory, remote);
            git('remote', 'add', 'origin', remote);
            if (moved) {
                git('-c', 'user.name=Security Test', '-c', 'user.email=test@example.invalid', 'commit', '--quiet', '--allow-empty', '-m', 'Moved synthetic tag');
                git('push', '--quiet', '--force', 'origin', 'HEAD:refs/tags/v1.2.3');
                git('checkout', '--quiet', '--detach', commit);
            }
            const result = spawnSync('bash', ['-c', runBlock(name)], {
                cwd: directory, encoding: 'utf8',
                env: { PATH: process.env.PATH, RELEASE_TAG: 'v1.2.3', EXPECTED_COMMIT_SHA: commit }
            });
            if (moved) {
                assert.notEqual(result.status, 0);
                assert.match(result.stderr, /Release tag moved/);
            } else {
                assert.equal(result.status, 0, result.stderr);
            }
        });
    }
}

test('restore commands override the image entrypoint before selecting the restore subcommand', () => {
    for (const name of ['restore.sh', 'update.sh']) {
        const script = readFileSync(join(root, 'deploy/scripts', name), 'utf8');
        assert.match(script, /docker compose run --rm --entrypoint \/app\/learnvault-server "\$CONTAINER" restore --backup/);
    }
});
