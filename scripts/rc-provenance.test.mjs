import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync, execFileSync } from 'node:child_process';
const root = resolve(import.meta.dirname, '..');
const producer = readFileSync(join(root, '.github/workflows/rust-server-rc.yml'), 'utf8');
const publisher = readFileSync(join(root, '.github/workflows/rust-server-rc-publish.yml'), 'utf8');
function block(source, name) {
    const step = source.split(`      - name: ${name}\n`)[1];
    assert.ok(step, `Missing workflow step ${name}`);
    const lines = step.split('        run: |\n')[1].split('\n');
    const end = lines.findIndex(line => line.trim() && !line.startsWith('          '));
    return lines.slice(0, end < 0 ? undefined : end).map(line => line.slice(10)).join('\n');
}
function fixture(t) {
    const directory = mkdtempSync(join(tmpdir(), 'learnvault-rc-'));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    mkdirSync(join(directory, 'bin'));
    mkdirSync(join(directory, 'dist/stage/web'), { recursive: true });
    for (const name of ['LICENSE', 'learnvault-server', 'web/200.html']) writeFileSync(join(directory, 'dist/stage', name), 'synthetic');
    writeFileSync(join(directory, 'bin/gh'), '#!/bin/sh\ncat "$FIXTURE_RUN_JSON"\n', { mode: 0o700 });
    const env = { PATH: `${directory}/bin:${process.env.PATH}`, HOME: process.env.HOME, TMPDIR: process.env.TMPDIR,
        GITHUB_REPOSITORY: 'example/LearnVault', GITHUB_RUN_ID: '42', RUN_ID: '42', VERSION: '0.8.1-rc.1',
        TARGET: 'x86_64-unknown-linux-musl', GITHUB_OUTPUT: join(directory, 'outputs'), FIXTURE_RUN_JSON: join(directory, 'run.json') };
    const git = (...args) => execFileSync('git', args, { cwd: directory, env, encoding: 'utf8' }).trim();
    git('init', '-q');
    git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-q', '--allow-empty', '-m', 'Synthetic RC');
    const sha = git('rev-parse', 'HEAD');
    const run = { id: 42, repository: { full_name: env.GITHUB_REPOSITORY }, head_repository: { full_name: env.GITHUB_REPOSITORY },
        path: '.github/workflows/rust-server-rc.yml', event: 'workflow_dispatch', status: 'completed', conclusion: 'success' };
    const execute = (source, name) => spawnSync('bash', ['-c', block(source, name)], { cwd: directory, env, encoding: 'utf8' });
    const packageResult = execute(producer, 'Package and checksum');
    assert.equal(packageResult.status, 0, packageResult.stderr);
    return { directory, env, sha, run, execute };
}
test('RC metadata records the actual checkout and publication consumes that source SHA', t => {
    const f = fixture(t);
    assert.equal(JSON.parse(readFileSync(join(f.directory, 'dist/rust-rc-metadata.json'), 'utf8')).source_sha, f.sha);
    const result = f.execute(publisher, 'Verify artifact before publishing');
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(f.env.GITHUB_OUTPUT, 'utf8').trim(), `source_sha=${f.sha}`);
    assert.match(block(publisher, 'Publish draft prerelease'), /-f "sha=\$SOURCE_SHA"/);
    assert.match(block(publisher, 'Publish draft prerelease'), /--verify-tag/);
    assert.match(producer, /path: \|[\s\S]*dist\/rust-rc-metadata\.json/);
});
for (const [field, value] of [['path', 'other.yml'], ['event', 'pull_request'], ['status', 'in_progress'], ['conclusion', 'failure'], ['id', 99], ['head_repository', { full_name: 'other/repo' }], ['repository', { full_name: 'other/repo' }], ['valid', null]]) {
    test(`RC run identity gate: ${field}`, t => {
        const f = fixture(t);
        if (field !== 'valid') f.run[field] = value;
        writeFileSync(f.env.FIXTURE_RUN_JSON, JSON.stringify(f.run));
        assert.equal(f.execute(publisher, 'Verify RC producer run').status === 0, field === 'valid');
    });
}
for (const field of ['version', 'repository', 'run_id', 'target', 'source_sha']) {
    test(`RC artifact rejects mismatched ${field} even with matching checksums`, t => {
        const f = fixture(t);
        const path = join(f.directory, 'dist/rust-rc-metadata.json');
        const metadata = JSON.parse(readFileSync(path, 'utf8')); metadata[field] = 'wrong';
        writeFileSync(path, JSON.stringify(metadata));
        execFileSync('bash', ['-c', 'cd dist && sha256sum learnvault-server-linux-x86_64.tar.gz rust-rc-metadata.json > SHA256SUMS.rust-rc'], { cwd: f.directory, env: f.env });
        assert.notEqual(f.execute(publisher, 'Verify artifact before publishing').status, 0);
    });
}
test('RC checksum file cannot select arbitrary local files', t => {
    const f = fixture(t);
    writeFileSync(join(f.directory, 'dist/SHA256SUMS.rust-rc'), `${'0'.repeat(64)}  /outside\n${'0'.repeat(64)}  rust-rc-metadata.json\n`);
    assert.notEqual(f.execute(publisher, 'Verify artifact before publishing').status, 0);
});

for (const status of [0, 1]) {
    test(`RC publication reserves a fresh source-bound tag: API status ${status}`, t => {
        const f = fixture(t);
        f.env.SOURCE_SHA = f.sha; f.env.TAG = 'rust-rc-synthetic';
        f.env.GH_LOG = join(f.directory, 'gh.log'); f.env.CREATE_REF_STATUS = String(status);
        writeFileSync(join(f.directory, 'bin/gh'), '#!/bin/sh\nprintf "%s\\n" "$*" >> "$GH_LOG"\nif [ "$1" = api ]; then exit "$CREATE_REF_STATUS"; fi\nprintf "synthetic-release-url\\n"\n', { mode: 0o700 });
        const result = f.execute(publisher, 'Publish draft prerelease');
        assert.equal(result.status === 0, status === 0);
        const log = readFileSync(f.env.GH_LOG, 'utf8');
        assert.ok(log.includes(`sha=${f.sha}`));
        assert.equal(log.includes('release create'), status === 0);
    });
}
