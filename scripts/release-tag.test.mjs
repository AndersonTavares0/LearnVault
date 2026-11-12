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

test('manual release repair only runs from main', t => {
    const { directory } = fixture(t);
    for (const [ref, expected] of [['refs/heads/main', 0], ['refs/heads/feature', 1]]) {
        const result = spawnSync('bash', ['-c', runBlock('Require the trusted release workflow ref')], {
            cwd: directory, encoding: 'utf8',
            env: { PATH: process.env.PATH, EVENT_NAME: 'workflow_dispatch', WORKFLOW_REF: ref }
        });
        assert.equal(result.status, expected, result.stderr);
    }
    const releaseEvent = spawnSync('bash', ['-c', runBlock('Require the trusted release workflow ref')], {
        cwd: directory, encoding: 'utf8',
        env: { PATH: process.env.PATH, EVENT_NAME: 'release', WORKFLOW_REF: 'refs/tags/v1.2.3' }
    });
    assert.equal(releaseEvent.status, 0, releaseEvent.stderr);
});

test('release repair rejects missing, draft, and prerelease targets without creating or publishing releases', t => {
    const { directory } = fixture(t);
    const script = runBlock('Require an existing published release of the tag\'s kind');
    const wrapped = `gh() { printf '%s' "$GH_RESULT"; }\n${script}`;
    const runWithFunction = metadata => spawnSync('bash', ['-c', wrapped], {
        cwd: directory, encoding: 'utf8',
        env: { PATH: process.env.PATH, GH_TOKEN: 'synthetic', GITHUB_REPOSITORY: 'owner/repo', RELEASE_TAG: 'v1.2.3', GH_RESULT: metadata }
    });
    const published = runWithFunction('{"isDraft":false,"isPrerelease":false}');
    assert.equal(published.status, 0, published.stderr);
    assert.notEqual(runWithFunction('{"isDraft":true,"isPrerelease":false}').status, 0);
    assert.notEqual(runWithFunction('{"isDraft":false,"isPrerelease":true}').status, 0);
    const missing = spawnSync('bash', ['-c', `gh() { return 1; }\n${script}`], {
        cwd: directory, encoding: 'utf8',
        env: { PATH: process.env.PATH, GH_TOKEN: 'synthetic', GITHUB_REPOSITORY: 'owner/repo', RELEASE_TAG: 'v1.2.3' }
    });
    assert.notEqual(missing.status, 0);
    assert.doesNotMatch(workflow, /gh release create|--draft=false/);
});

test('updater notes use the selected release body with a fallback and repair leaves release notes untouched', t => {
    const { directory } = fixture(t);
    const script = runBlock('Resolve release notes');
    const wrapped = `gh() { printf '%s' "$GH_RESULT"; }\n${script}`;
    const run = (body) => {
        const output = join(directory, `output-${body ? 'body' : 'empty'}`);
        const result = spawnSync('bash', ['-c', wrapped], {
            cwd: directory, encoding: 'utf8',
            env: {
                PATH: process.env.PATH,
                GH_TOKEN: 'synthetic',
                GITHUB_REPOSITORY: 'owner/repo',
                RELEASE_TAG: 'v1.2.3',
                GH_RESULT: body,
                GITHUB_OUTPUT: output
            }
        });
        assert.equal(result.status, 0, result.stderr);
        const line = readFileSync(output, 'utf8').trim();
        const encoded = line.slice('release_notes_base64='.length);
        return Buffer.from(encoded, 'base64').toString('utf8');
    };

    assert.equal(run('## What changed\n\n- Synthetic updater notes'), '## What changed\n\n- Synthetic updater notes');
    assert.equal(run(''), 'LearnVault release v1.2.3');
    assert.match(workflow, /--json body --jq '\.body \/\/ ""'/);
    assert.match(workflow, /- name: Attribute release contributors\n\s+if: \$\{\{ github\.event_name == 'release' \}\}/);
});

test('manual repair does not move latest or publish before the post-build tag check', () => {
    assert.match(workflow, /flavor: latest=false/);
    assert.match(workflow, /type=raw,value=latest,enable=\$\{\{ github\.event_name == 'release' && !contains\(needs\.validate-tag\.outputs\.tag, '-'\) \}\}/);
    const containerCheck = workflow.indexOf('- name: Confirm container release tag did not move');
    const containerPush = workflow.indexOf('- name: Build and push the image by digest');
    const containerScan = workflow.indexOf('- name: Scan the pushed image for HIGH and CRITICAL vulnerabilities');
    const containerVerify = workflow.indexOf('- name: Check the signature before publishing the release');
    const containerTags = workflow.indexOf('- name: Apply the release tags to the verified image');
    assert.ok(containerCheck < containerPush && containerPush < containerScan && containerScan < containerVerify && containerVerify < containerTags, 'container tag is checked before push, and tags are applied only after the scan and the signature check');

    const checksum = workflow.indexOf('- name: Create SHA256SUMS');
    const assetCheck = workflow.indexOf('- name: Confirm release tag did not move');
    const upload = workflow.indexOf('- name: Upload assets to the published release');
    assert.ok(checksum < assetCheck && assetCheck < upload, 'asset tag is checked immediately before upload');
});

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

test('release tags accept strict stable and beta versions only', t => {
    const { directory } = fixture(t);
    const output = join(directory, 'github-output');
    const block = runBlock('Require a strict vX.Y.Z or vX.Y.Z-beta.N tag');
    for (const [tag, accepted] of [['v0.9.0', true], ['v0.9.0-beta.0', true], ['v0.9.0-beta.1', true], ['v0.9.0-beta.01', false], ['v0.9.0-beta', false], ['v0.9.0-rc.1', false], ['v0.9-beta.1', false], ['v00.9.0', false]]) {
        const result = spawnSync('bash', ['-c', block], {
            cwd: directory, encoding: 'utf8',
            env: { PATH: process.env.PATH, RELEASE_TAG: tag, GITHUB_OUTPUT: output }
        });
        assert.equal(result.status === 0, accepted, `${tag}: ${result.stderr}`);
    }
});

test('beta releases never move stable, latest or minor image tags', () => {
    const stableOnly = "!contains(needs.validate-tag.outputs.tag, '-')";
    assert.equal(workflow.split(stableOnly).length - 1, 3);
    assert.match(workflow, /expected_prerelease=true/);
});

test('the cosign identity accepts stable and beta release tags, and nothing else', () => {
    const pattern = workflow.match(/@(refs\/\(.+?\))\$"/)?.[1];
    assert.ok(pattern, 'cosign identity pattern not found');
    // YAML and bash leave one backslash per `\\` in the source text.
    const identity = new RegExp(`^${pattern.replaceAll('\\\\', '\\')}$`);
    for (const ref of ['refs/tags/v0.9.0', 'refs/tags/v0.9.0-beta.1', 'refs/tags/v1.0.0-beta.10', 'refs/heads/main']) assert.match(ref, identity);
    for (const ref of ['refs/tags/v0.9.0-beta.01', 'refs/tags/v0.9.0-rc.1', 'refs/tags/v0.9.0-beta', 'refs/heads/feature']) assert.doesNotMatch(ref, identity);
});
