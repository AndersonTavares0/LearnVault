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

test('release repair rejects missing and draft targets, and reports pre-releases, without creating or publishing releases', t => {
    const { directory } = fixture(t);
    const script = runBlock('Require an existing published release');
    const wrapped = `gh() { printf '%s' "$GH_RESULT"; }\n${script}`;
    const runWithFunction = metadata => spawnSync('bash', ['-c', wrapped], {
        cwd: directory, encoding: 'utf8',
        env: { PATH: process.env.PATH, GH_TOKEN: 'synthetic', GITHUB_REPOSITORY: 'owner/repo', RELEASE_TAG: 'v1.2.3', GH_RESULT: metadata, GITHUB_OUTPUT: join(directory, 'kind-output') }
    });
    const published = runWithFunction('{"isDraft":false,"isPrerelease":false}');
    assert.equal(published.status, 0, published.stderr);
    assert.notEqual(runWithFunction('{"isDraft":true,"isPrerelease":false}').status, 0);
    assert.equal(runWithFunction('{"isDraft":false,"isPrerelease":true}').status, 0);
    assert.notEqual(runWithFunction('{"isDraft":false}').status, 0);
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
    assert.match(workflow, /type=raw,value=latest,enable=\$\{\{ github\.event_name == 'release' && needs\.validate-tag\.outputs\.prerelease == 'false' \}\}/);
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

test('release tags accept strict stable versions only', t => {
    const { directory } = fixture(t);
    const output = join(directory, 'github-output');
    const block = runBlock('Require a strict vX.Y.Z tag');
    for (const [tag, accepted] of [['v0.9.0', true], ['v0.9.0-rc.0', false], ['v0.9.0-rc.1', false], ['v0.9.0-rc.01', false], ['v0.9.0-rc', false], ['v0.9.0-beta.1', false], ['v0.9-rc.1', false], ['v00.9.0', false]]) {
        const result = spawnSync('bash', ['-c', block], {
            cwd: directory, encoding: 'utf8',
            env: { PATH: process.env.PATH, RELEASE_TAG: tag, GITHUB_OUTPUT: output }
        });
        assert.equal(result.status === 0, accepted, `${tag}: ${result.stderr}`);
    }
});

test('release smoke is tied to trusted helper code and independent tagged version metadata', () => {
    assert.match(workflow, /ref: \$\{\{ github\.workflow_sha \}\}/);
    assert.match(workflow, /\.github\/release-please\/version\.txt/);
    assert.match(workflow, /"v\$release_version" != "\$RELEASE_TAG"/);
    for (const arch of ['amd64', 'arm64']) {
        const block = runBlock(`Smoke test packaged Linux ${arch} binary`);
        assert.match(block, /\.\/\.trusted-workflow\/scripts\/smoke-release-binary\.sh/);
        assert.match(block, new RegExp(`learnvault-linux-${arch}\\.tar\\.gz`));
    }
    const smokeIndex = workflow.indexOf('- name: Smoke test the pushed ${{ matrix.arch }} image');
    const digestIndex = workflow.indexOf('- name: Keep the scanned digest for the index');
    assert.ok(smokeIndex >= 0 && smokeIndex < digestIndex, 'image smoke gates the digest accepted by the publish job');
    assert.match(workflow.slice(smokeIndex, digestIndex), /docker pull "\$IMAGE"/);
    assert.match(workflow.slice(smokeIndex, digestIndex), /RELEASE_TAG: \$\{\{ needs\.validate-tag\.outputs\.tag \}\}/);
});

test('real-server CI smoke runs after the SPA build and retains Playwright cache and ffmpeg setup', () => {
    const ci = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8');
    const e2eStart = ci.indexOf('  e2e-server:');
    const e2eBlock = ci.slice(e2eStart);
    const smokeIndex = e2eBlock.indexOf('- name: Smoke test real server binary');
    assert.ok(smokeIndex >= 0 && e2eBlock.indexOf('pnpm --filter learnvault-web-next build') < smokeIndex);
    assert.match(e2eBlock, /name: Restore Cargo registry and target/);
    assert.match(e2eBlock, /cache: pnpm/);
    assert.match(e2eBlock, /name: Install system ffmpeg for transcoding E2E/);
    assert.match(e2eBlock, /pnpm exec playwright install chromium/);
});

test('release publishes the source SBOM, keyless checksum bundle, and binary attestations', () => {
    const publish = workflow.slice(workflow.indexOf('\n  publish:\n'));
    assert.match(publish, /id-token: write/);
    assert.match(publish, /attestations: write/);
    assert.match(publish, /scan-type: fs[\s\S]*?format: cyclonedx[\s\S]*?output: dist\/learnvault-/);
    assert.match(publish, /cosign sign-blob --yes --bundle dist\/SHA256SUMS\.sigstore\.json dist\/SHA256SUMS/);
    assert.match(publish, /uses: actions\/attest-build-provenance@[0-9a-f]{40}/);
    assert.match(publish, /uses: actions\/attest-sbom@[0-9a-f]{40}/);
    assert.ok(publish.indexOf('- name: Sign SHA256SUMS (keyless)') < publish.indexOf('- name: Attest build provenance'));
    assert.ok(publish.indexOf('- name: Attest SBOM for release binaries') < publish.indexOf('- name: Upload assets to the published release'));

    const process = readFileSync(join(root, 'docs/releases/process.md'), 'utf8');
    for (const command of ['cosign verify-blob SHA256SUMS', 'sha256sum -c SHA256SUMS --ignore-missing', 'gh attestation verify learnvault-linux-amd64.tar.gz', '--predicate-type https://cyclonedx.org/bom']) {
        assert.ok(process.includes(command), `release instructions include ${command}`);
    }
    assert.ok(process.includes('github[.]com'));
    assert.equal(process.includes('github\\\\.com'), false, 'consumer regex must not double-escape dots');
    assert.match(workflow, /"v\$release_version" != "\$RELEASE_TAG"/);
});

test('pre-releases never move stable, latest or minor image tags and no rc tag is applied', () => {
    const stableOnly = "needs.validate-tag.outputs.prerelease == 'false'";
    assert.equal(workflow.split(stableOnly).length - 1, 6);
    assert.doesNotMatch(workflow, /value=rc\b/);
});

test('the cosign identity accepts stable release tags, and nothing else', () => {
    const pattern = workflow.match(/@(refs\/\(.+?\))\$"/)?.[1];
    assert.ok(pattern, 'cosign identity pattern not found');
    // YAML and bash leave one backslash per `\\` in the source text.
    const identity = new RegExp(`^${pattern.replaceAll('\\\\', '\\')}$`);
    for (const ref of ['refs/tags/v0.9.0', 'refs/tags/v1.0.0', 'refs/heads/main']) assert.match(ref, identity);
    for (const ref of ['refs/tags/v0.9.0-rc.1', 'refs/tags/v0.9.0-rc.01', 'refs/tags/v0.9.0-rc', 'refs/tags/v0.9.0-beta.1', 'refs/heads/feature']) assert.doesNotMatch(ref, identity);
});

test('pushing and publishing wait for approval in the release environment', () => {
    for (const job of ['container-approval:', 'publish:']) {
        const start = workflow.indexOf(`\n  ${job}\n`);
        assert.ok(start >= 0, `missing job ${job}`);
        const next = workflow.slice(start + 1).search(/\n {2}[a-z-]+:\n/);
        const block = workflow.slice(start, next < 0 ? undefined : start + 1 + next);
        assert.match(block, /\n    environment: release\n/, `${job} must use the release environment`);
    }
});

test('post-publication verification binds this run digest to targeted aliases and handles partial mutation', () => {
    const start = workflow.indexOf('\n  verify-published:\n');
    assert.ok(start >= 0, 'missing Verify published release job');
    const verify = workflow.slice(start);
    assert.match(verify, /always\(\)/);
    assert.match(verify, /needs\.container\.outputs\.tag_move_started == 'true'/);
    assert.match(verify, /needs\.publish\.outputs\.asset_upload_started == 'true'/);
    assert.match(
        verify,
        /\(needs\.publish\.outputs\.asset_upload_started == 'true' \|\|\s*\(needs\.publish\.result != 'failure' &&\s*\(needs\.container\.outputs\.tag_move_started == 'true'/,
        'a rejected publish environment approval must not start verification'
    );
    assert.match(workflow, /index_digest: \$\{\{ steps\.index\.outputs\.digest \}\}/);
    assert.match(workflow, /image_tags: \$\{\{ steps\.meta\.outputs\.tags \}\}/);
    assert.match(verify, /EXPECTED_DIGEST: \$\{\{ needs\.container\.outputs\.index_digest \}\}/);
    assert.match(verify, /docker buildx imagetools inspect --format '\{\{\.Manifest\.Digest\}\}'/);
    assert.match(verify, /observed" != "\$EXPECTED_DIGEST/);
    assert.match(verify, /cosign verify "\$\{IMAGE\}@\$\{EXPECTED_DIGEST\}"/);
    assert.doesNotMatch(verify, /cosign verify "\$IMAGE:\$\{alias\}"/);
    assert.match(verify, /manifest unknown\|no such manifest\|404 Not Found/);
    assert.match(verify, /sleep 5/);
    assert.match(verify, /verify-release-index-attestations\.mjs/);
    assert.match(verify, /sha256sum -c SHA256SUMS/);
    assert.match(verify, /"learnvault-\$\{RELEASE_TAG\}\.sbom\.cdx\.json"/);
    assert.match(verify, /"SHA256SUMS\.sigstore\.json"/);
    assert.match(verify, /cosign verify-blob verify-dist\/SHA256SUMS/);
    assert.match(verify, /--certificate-identity-regexp "\^https:\/\/github\[\.\]com\/\$\{REPOSITORY\/\/\.\/\[\.\]\}/);
    const checksumVerification = verify.slice(verify.indexOf('cosign verify-blob'), verify.indexOf('error_file="$(mktemp)"'));
    assert.doesNotMatch(checksumVerification, /\\\\\./, 'checksum verification must not retain doubled escape sequences');
    assert.match(verify, /gh attestation verify verify-dist\/learnvault-linux-amd64\.tar\.gz/);
    assert.match(verify, /--predicate-type https:\/\/slsa\.dev\/provenance\/v1/);
    assert.match(verify, /HTTP 404\|404 Not Found\|status code: 404/);
    assert.match(verify, /Alert on partial publication or verification failure/);
    assert.doesNotMatch(verify, /^  concurrency:/m, 'verification remains inside the Release run and its existing concurrency lock');
});

test('partial-publication alert does not run rollback path text as shell command substitution', () => {
    const verify = workflow.slice(workflow.indexOf('\n  verify-published:\n'));
    const alert = verify.slice(verify.indexOf('- name: Alert on partial publication or verification failure'));
    const run = alert.match(/\n        run: \|\n([\s\S]*?)(?=\n  [a-z-]+:\n|$)/)?.[1];
    assert.ok(run, 'alert step run block not found');
    assert.doesNotMatch(run, /"[^"\n]*(?<!\\)`[^"\n]*"/, 'unescaped backticks inside double-quoted strings invoke command substitution');
    assert.match(run, /\.github\/workflows\/rollback\.yml/);
});

test('a release must be published and not a draft; a pre-release is flagged so it keeps the moving tags', t => {
    const { directory } = fixture(t);
    const script = runBlock('Require an existing published release');
    const output = join(directory, 'kind-output');
    const wrapped = `gh() { printf '%s' "$GH_RESULT"; }\n${script}`;
    const run = (tag, metadata) => spawnSync('bash', ['-c', wrapped], {
        cwd: directory, encoding: 'utf8',
        env: { PATH: process.env.PATH, GH_TOKEN: 'synthetic', GITHUB_REPOSITORY: 'owner/repo', RELEASE_TAG: tag, GH_RESULT: metadata, GITHUB_OUTPUT: output }
    });
    assert.notEqual(run('v0.9.0', '{"isDraft":true,"isPrerelease":true}').status, 0);
    assert.notEqual(run('v0.9.0', '{"isDraft":true,"isPrerelease":false}').status, 0);
    assert.equal(run('v0.9.0', '{"isDraft":false,"isPrerelease":false}').status, 0);
    assert.equal(run('v0.9.0', '{"isDraft":false,"isPrerelease":true}').status, 0);
    assert.match(readFileSync(output, 'utf8'), /prerelease=false\nprerelease=true\n$/);
});
