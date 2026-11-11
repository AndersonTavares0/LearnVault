import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';

const root = resolve(import.meta.dirname, '..');
const daemon = readFileSync(join(root, 'deploy/scripts/updater-daemon.sh'), 'utf8');
const functions = daemon.slice(daemon.indexOf('# Return minutes'), daemon.indexOf('if [ "$AUTO_UPDATE"'));

for (const [window, clock, status] of [
    ['03:15-05:30', '03:14', 1], ['03:15-05:30', '03:15', 0],
    ['03:15-05:30', '05:29', 0], ['03:15-05:30', '05:30', 1],
    ['23:30-01:15', '23:29', 1], ['23:30-01:15', '23:30', 0],
    ['23:30-01:15', '00:45', 0], ['23:30-01:15', '01:15', 1],
    ['09:00-10:00', '09:08', 0], ['24:00-05:00', '03:00', 2],
    ['03:60-05:00', '03:00', 2], ['03:00-03:00', '03:00', 2],
    ['3:00-05:00', '03:00', 2], ['bad', '03:00', 2],
]) {
    test(`maintenance window ${window} at ${clock} returns ${status}`, () => {
        const result = spawnSync('sh', ['-c', `${functions}\nin_window "$1" "$2"`, 'test', window, clock]);
        assert.equal(result.status, status, result.stderr?.toString());
    });
}

function fixture(t, settings = {}) {
    const directory = mkdtempSync(join(tmpdir(), 'learnvault-updater-'));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    const bin = join(directory, 'bin');
    mkdirSync(bin);
    writeFileSync(join(bin, 'docker'), `#!/bin/sh
printf '%s|%s\\n' "\${LEARNVAULT_IMAGE:-}" "$*" >> "$TEST_LOG"
case "$*" in
  'compose exec -T learnvault wget'*)
    if [ ! -f "$TEST_COUNTER" ]; then
      touch "$TEST_COUNTER"
      printf '{"version":"v1.0.0"}'
    else
      printf '{"version":"%s"}' "$TEST_ACTIVE_VERSION"
    fi ;;
  'compose exec -T learnvault /app/learnvault-server backup'*) exit "$TEST_BACKUP_EXIT" ;;
  'pull '*) exit "$TEST_PULL_EXIT" ;;
  'inspect --format={{.Image}} learnvault') printf 'sha256:previous' ;;
  'inspect --format='*) printf 'healthy' ;;
  'compose up -d --no-build '*) exit "$TEST_START_EXIT" ;;
esac
`, { mode: 0o700 });
    writeFileSync(join(bin, 'sleep'), '#!/bin/sh\nexit 99\n', { mode: 0o700 });
    // A stand-in for cosign: it records how it was called and exits with TEST_COSIGN_EXIT.
    const cosignLog = join(directory, 'cosign.log');
    writeFileSync(cosignLog, '');
    if (!settings.NO_COSIGN) writeFileSync(join(bin, 'cosign'), '#!/bin/sh\nprintf \'%s\\n\' "$*" >> "$TEST_COSIGN_LOG"\nif [ "${TEST_COSIGN_EXIT:-0}" != 0 ]; then echo "no matching signatures" >&2; exit "$TEST_COSIGN_EXIT"; fi\nprintf \'[{"critical":{"image":{"docker-manifest-digest":"sha256:%s"}}}]\' "${TEST_COSIGN_DIGEST}"\n', { mode: 0o700 });
    const log = join(directory, 'docker.log');
    writeFileSync(log, '');
    const env = {
        PATH: `${bin}:${process.env.PATH}`, HOME: directory,
        DATA_DIR: directory, TEST_LOG: log, TEST_COUNTER: join(directory, 'counter'),
        TEST_COSIGN_LOG: cosignLog, TEST_COSIGN_EXIT: '0', TEST_COSIGN_DIGEST: 'a'.repeat(64),
        TEST_ACTIVE_VERSION: 'v1.0.1', TEST_START_EXIT: '0', TEST_PULL_EXIT: '0', TEST_BACKUP_EXIT: '0',
        ...settings,
    };
    return { env, log, directory, cosignLog };
}

for (const settings of [
    { AUTO_UPDATE_WINDOW: '25:00-01:00' },
    { AUTO_UPDATE_WINDOW: '03:00-03:00' },
    { CHECK_INTERVAL_SECONDS: '0' },
    { CHECK_INTERVAL_SECONDS: '-1' },
    { CHECK_INTERVAL_SECONDS: '1.5' },
    { CHECK_INTERVAL_SECONDS: '000' },
    { CHECK_INTERVAL_SECONDS: '2147483648' },
    { CHECK_INTERVAL_SECONDS: '10000000000' },
]) {
    test(`invalid daemon configuration fails before Docker: ${JSON.stringify(settings)}`, t => {
        const fixtureData = fixture(t, { AUTO_UPDATE: 'true', ...settings });
        const result = spawnSync('sh', [join(root, 'deploy/scripts/updater-daemon.sh')], { env: fixtureData.env, timeout: 2000 });
        assert.equal(result.status, 1);
        assert.equal(readFileSync(fixtureData.log, 'utf8'), '');
    });
}

test('automatic updates remain opt-in even when settings are invalid', t => {
    const fixtureData = fixture(t, { AUTO_UPDATE: 'false', CHECK_INTERVAL_SECONDS: '0' });
    const result = spawnSync('sh', [join(root, 'deploy/scripts/updater-daemon.sh')], { env: fixtureData.env, timeout: 2000 });
    assert.equal(result.status, 99);
    assert.equal(readFileSync(fixtureData.log, 'utf8'), '');
});

for (const [name, settings, expectedStatus, rollback] of [
    ['matching target', {}, 0, false],
    ['wrong healthy version', { TEST_ACTIVE_VERSION: '1.0.0' }, 1, true],
    ['missing healthy version', { TEST_ACTIVE_VERSION: '' }, 1, true],
    ['startup failure', { TEST_START_EXIT: '1' }, 1, true],
    ['download failure', { TEST_PULL_EXIT: '1' }, 1, false],
    ['backup failure', { TEST_BACKUP_EXIT: '1' }, 1, false],
    ['unsigned image', { TEST_COSIGN_EXIT: '1' }, 1, false],
    ['missing cosign', { NO_COSIGN: '1' }, 1, false],
    ['unknown verification setting', { LEARNVAULT_VERIFY_SIGNATURE: 'True' }, 1, false],
    ['malformed signer repository', { LEARNVAULT_GITHUB_REPO: 'a(b)|c' }, 1, false],
]) {
    test(`update ${name}`, t => {
        const fixtureData = fixture(t, settings);
        const result = spawnSync('sh', [join(root, 'deploy/scripts/update.sh'), '1.0.1'], {
            env: fixtureData.env, encoding: 'utf8', timeout: 2000,
        });
        assert.equal(result.status, expectedStatus, result.stderr);
        const calls = readFileSync(fixtureData.log, 'utf8');
        if (rollback) {
            assert.match(calls, /sha256:previous\|compose run --rm --entrypoint \/app\/learnvault-server learnvault restore --backup/);
            assert.match(calls, /sha256:previous\|compose up -d learnvault/);
        } else {
            assert.doesNotMatch(calls, /compose run/);
        }
        if (settings.TEST_PULL_EXIT || settings.TEST_BACKUP_EXIT) assert.doesNotMatch(calls, /compose up/);
        // A target that cannot be verified is refused before any backup, download or change.
        if (settings.TEST_COSIGN_EXIT || settings.NO_COSIGN || settings.LEARNVAULT_VERIFY_SIGNATURE || settings.LEARNVAULT_GITHUB_REPO) assert.doesNotMatch(calls, /backup|pull |compose up/);
        if (settings.LEARNVAULT_VERIFY_SIGNATURE || settings.LEARNVAULT_GITHUB_REPO) return; // refused as a bad setting, before any history entry
        const history = JSON.parse(readFileSync(join(fixtureData.directory, 'update-history.json'), 'utf8'));
        assert.equal(history[0].status, expectedStatus === 0 ? 'success' : rollback ? 'rollback' : (settings.TEST_COSIGN_EXIT || settings.NO_COSIGN) ? 'failed_verification' : (settings.LEARNVAULT_VERIFY_SIGNATURE || settings.LEARNVAULT_GITHUB_REPO) ? null : settings.TEST_PULL_EXIT ? 'failed_download' : 'failed_pre_backup');
    });
}

test('the target image is verified against the release workflow identity', t => {
    const fixtureData = fixture(t);
    const result = spawnSync('sh', [join(root, 'deploy/scripts/update.sh'), '1.0.1'], { env: fixtureData.env, encoding: 'utf8', timeout: 2000 });
    assert.equal(result.status, 0, result.stderr);
    const call = readFileSync(fixtureData.cosignLog, 'utf8');
    assert.match(call, /^verify ghcr\.io\/andersontavares0\/learnvault:1\.0\.1 /);
    assert.match(call, /--certificate-oidc-issuer https:\/\/token\.actions\.githubusercontent\.com/);
    assert.match(call, /--certificate-identity-regexp \^https:\/\/github\\\.com\/AndersonTavares0\/LearnVault\/\\\.github\/workflows\/release\\\.yml@refs\//);
});

test('the image that is pulled and started is the verified digest, not the tag', t => {
    const fixtureData = fixture(t);
    const result = spawnSync('sh', [join(root, 'deploy/scripts/update.sh'), '1.0.1'], { env: fixtureData.env, encoding: 'utf8', timeout: 2000 });
    assert.equal(result.status, 0, result.stderr);
    const calls = readFileSync(fixtureData.log, 'utf8');
    const digest = `ghcr.io/andersontavares0/learnvault@sha256:${'a'.repeat(64)}`;
    assert.match(calls, new RegExp(`pull ${digest}`));
    assert.match(calls, new RegExp(`${digest}\\|compose up -d --no-build`));
    assert.doesNotMatch(calls, /learnvault:1\.0\.1/);
});

test('the signer identity accepts only the release workflow of this repository', () => {
    const script = readFileSync(join(root, 'deploy/scripts/update.sh'), 'utf8');
    const fn = script.slice(script.indexOf('signer_identity_regex() {'), script.indexOf('log() {'));
    const regex = spawnSync('sh', ['-c', `${fn}\nsigner_identity_regex "$1"`, 'test', 'AndersonTavares0/LearnVault'], { encoding: 'utf8' }).stdout;
    const matches = id => spawnSync('grep', ['-Eq', regex], { input: id }).status === 0;
    const base = 'https://github.com/AndersonTavares0/LearnVault/.github/workflows/release.yml';
    assert.ok(matches(`${base}@refs/tags/v0.9.0`));
    assert.ok(matches(`${base}@refs/heads/main`));
    assert.ok(!matches(`${base}@refs/heads/feature`));
    assert.ok(!matches(`${base}@refs/tags/v0.9`));
    assert.ok(!matches(base.replace('release.yml', 'ci.yml') + '@refs/tags/v0.9.0'));
    assert.ok(!matches(base.replace('AndersonTavares0', 'Evil') + '@refs/tags/v0.9.0'));
    assert.ok(!matches(base.replace('LearnVault', 'LearnVault2') + '@refs/tags/v0.9.0'));
    assert.ok(!matches(`x${base}@refs/tags/v0.9.0`));
});

test('signature checking can be turned off for a private mirror', t => {
    const fixtureData = fixture(t, { LEARNVAULT_VERIFY_SIGNATURE: 'false', TEST_COSIGN_EXIT: '1' });
    const result = spawnSync('sh', [join(root, 'deploy/scripts/update.sh'), '1.0.1'], { env: fixtureData.env, encoding: 'utf8', timeout: 2000 });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(fixtureData.cosignLog, 'utf8'), '');
});

test('an existing update lock prevents a second attempt from touching Docker', t => {
    const fixtureData = fixture(t);
    mkdirSync(join(fixtureData.directory, '.update.lock'));
    const result = spawnSync('sh', [join(root, 'deploy/scripts/update.sh'), '1.0.1'], {
        env: fixtureData.env, encoding: 'utf8', timeout: 2000,
    });
    assert.equal(result.status, 1);
    assert.equal(readFileSync(fixtureData.log, 'utf8'), '');
});

for (const interval of ['1', '0008', '2147483647']) {
    test(`valid daemon interval ${interval} reaches the polling loop`, t => {
        const fixtureData = fixture(t, { AUTO_UPDATE: 'true', CHECK_INTERVAL_SECONDS: interval });
        const result = spawnSync('sh', [join(root, 'deploy/scripts/updater-daemon.sh')], {
            env: fixtureData.env, timeout: 2000,
        });
        assert.equal(result.status, 99);
        assert.match(readFileSync(fixtureData.log, 'utf8'), /compose version/);
    });
}
