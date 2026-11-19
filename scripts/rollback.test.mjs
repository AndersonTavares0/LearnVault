import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import {
    applyPlan, assertNoActiveRelease, buildPlan, isLegacyTag, canDeleteStaging, certIdentityRegex, formatPlan, parseTag, planAliases, platformsOf, validateRecorded,
} from './rollback.mjs';

const root = resolve(import.meta.dirname, '..');
const rollbackYml = readFileSync(join(root, '.github/workflows/rollback.yml'), 'utf8');
const releaseYml = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');

const REPO = 'AndersonTavares0/LearnVault';
const IMAGE = 'ghcr.io/andersontavares0/learnvault';
const d = char => `sha256:${char.repeat(64)}`;
const GOOD = d('a'); // v0.8.0 index, amd64 only
const BAD = d('b'); // v0.9.5 index, amd64 + arm64
const AMD_GOOD = d('1');
const AMD_BAD = d('2');
const ARM_BAD = d('3');
const ATT = d('4');

const indexOf = (children, withAttestation = true) => JSON.stringify({
    manifests: [
        ...children.map(([arch, digest]) => ({ digest, platform: { os: 'linux', architecture: arch } })),
        ...(withAttestation ? [{ digest: ATT, platform: { os: 'unknown', architecture: 'unknown' } }] : []),
    ],
});

// A synthetic GHCR + GitHub backend that answers the exact commands the script issues.
function backend({ tags, signed = [GOOD, BAD], draft = false, releases, activeRuns = {}, cosignError } = {}) {
    const state = {
        tags: { ...tags },
        versions: new Map(),
        releases: releases ?? { 'v0.8.0': { isPrerelease: false }, 'v0.9.5': { isPrerelease: false } },
        latest: 'v0.9.5',
        log: [],
        raw: {
            [GOOD]: indexOf([['amd64', AMD_GOOD]]),
            [BAD]: indexOf([['amd64', AMD_BAD], ['arm64', ARM_BAD]]),
        },
        nextVersionId: 100,
    };
    const manifestExists = digest => digest in state.raw || [AMD_GOOD, AMD_BAD, ARM_BAD].includes(digest);
    const tombstones = {};
    const run = (command, args) => {
        state.log.push([command, ...args]);
        const ok = stdout => ({ status: 0, stdout, stderr: '' });
        const fail = stderr => ({ status: 1, stdout: '', stderr });
        if (command === 'docker' && args[2] === 'inspect' && args[3] === '--raw') {
            const digest = args[4].split('@')[1];
            return state.raw[digest] ? ok(state.raw[digest]) : fail('not found');
        }
        if (command === 'docker' && args[2] === 'inspect') {
            const [, ref] = [null, args[3]];
            if (ref.includes('@')) return manifestExists(ref.split('@')[1]) ? ok(`${ref.split('@')[1]}\n`) : fail('not found');
            const tag = ref.split(':')[1];
            if (tag === 'flaky') return fail('i/o timeout');
            return state.tags[tag] ? ok(`${state.tags[tag]}\n`) : fail(`${ref}: not found`);
        }
        if (command === 'docker' && args[2] === 'create') {
            const annotation = args.includes('--annotation');
            const source = args.at(-1).split('@')[1];
            for (let i = 0; i < args.length; i += 1) {
                if (args[i] !== '--tag') continue;
                const tag = args[i + 1].split(':')[1];
                if (annotation) {
                    tombstones[tag] = `sha256:${createHash('sha256').update(`${tag}${source}`).digest('hex')}`;
                    state.raw[tombstones[tag]] = state.raw[source];
                    state.tags[tag] = tombstones[tag];
                } else {
                    state.tags[tag] = source;
                }
            }
            return ok('');
        }
        if (command === 'cosign') {
            const digest = args[1].split('@')[1];
            assert.equal(args[args.indexOf('--certificate-identity-regexp') + 1], certIdentityRegex(REPO));
            if (cosignError) return fail(cosignError);
            return signed.includes(digest) ? ok('') : fail('Error: no signatures found');
        }
        if (command === 'gh' && args[0] === 'run' && args[1] === 'list') {
            assert.equal(args[args.indexOf('--workflow') + 1], 'release.yml');
            const status = args[args.indexOf('--status') + 1];
            if (activeRuns.error) return fail('HTTP 500');
            return ok(JSON.stringify((activeRuns[status] ?? []).map(databaseId => ({ databaseId }))));
        }
        if (command === 'gh' && args[0] === 'release' && args[1] === 'view') {
            const release = state.releases[args[2]];
            return release ? ok(JSON.stringify({ tagName: args[2], isDraft: draft, isPrerelease: release.isPrerelease })) : fail('release not found');
        }
        if (command === 'gh' && args[0] === 'release' && args[1] === 'edit') {
            const tag = args[2];
            if (args.includes('--prerelease')) { state.releases[tag].isPrerelease = true; if (state.latest === tag) state.latest = null; }
            if (args.includes('--latest')) state.latest = tag;
            return ok('');
        }
        if (command === 'gh' && args[0] === 'api' && args[1] === `repos/${REPO}/releases/latest`) return ok(`${state.latest}\n`);
        if (command === 'gh' && args[0] === 'api' && args[1] === `users/andersontavares0`) return ok('User\n');
        if (command === 'gh' && args[0] === 'api' && args.includes('--paginate')) {
            // Versions grouped by digest, each carrying every tag that points at it.
            const byDigest = new Map();
            for (const [tag, digest] of Object.entries(state.tags)) byDigest.set(digest, [...(byDigest.get(digest) ?? []), tag]);
            return ok([...byDigest].map(([digest, versionTags]) => {
                if (!state.versions.has(digest)) state.versions.set(digest, state.nextVersionId++);
                return JSON.stringify({ id: state.versions.get(digest), tags: versionTags });
            }).join('\n'));
        }
        if (command === 'gh' && args[0] === 'api' && args[1] === '-X' && args[2] === 'DELETE') {
            const id = Number(args[3].split('/').pop());
            const digest = [...state.versions].find(([, versionId]) => versionId === id)?.[0];
            for (const tag of Object.keys(state.tags)) if (state.tags[tag] === digest) delete state.tags[tag];
            state.log.push(['DELETED', digest]);
            return ok('');
        }
        throw new Error(`Unexpected command: ${command} ${args.join(' ')}`);
    };
    return { run, state };
}

const planEnv = (recorded, extra = {}) => ({ repository: REPO, targetTag: 'v0.8.0', badTag: 'v0.9.5', recorded, ...extra });
const mutations = state => state.log.filter(entry => entry[0] === 'DELETED' || (entry[1] === 'buildx' && entry[3] === 'create') || (entry[0] === 'gh' && entry[2] === 'edit'));

// State after the bad release: v0.8.0 amd64-only; the record taken before it had latest=GOOD and stable/0.8/0.9 absent.
const REAL = { 'latest': BAD, '0.8.0': GOOD, '0.9.5': BAD, '0.9': BAD, 'stable': BAD };
const REAL_RECORD = { 'latest': GOOD, 'stable': null, '0.8': null, '0.9': null };

test('only runnable platforms count: amd64-only v0.8.0 is a valid target, an empty index is not', () => {
    assert.deepEqual(platformsOf(indexOf([['amd64', AMD_GOOD]])), { 'linux/amd64': AMD_GOOD });
    assert.deepEqual(Object.keys(platformsOf(indexOf([['amd64', AMD_BAD], ['arm64', ARM_BAD]]))), ['linux/amd64', 'linux/arm64']);
    assert.throws(() => platformsOf(indexOf([])), /no runnable platform/);
    assert.throws(() => platformsOf('{}'), /Not an image index/);
});

test('cosign identity regex uses single escaping and matches only release.yml on a tag or main', () => {
    const regex = new RegExp(certIdentityRegex(REPO));
    assert.equal(certIdentityRegex(REPO), String.raw`^https://github\.com/AndersonTavares0/LearnVault/\.github/workflows/release\.yml@refs/(tags/v[0-9]+\.[0-9]+\.[0-9]+|heads/main)$`);
    const base = `https://github.com/${REPO}/.github/workflows/release.yml`;
    assert.ok(regex.test(`${base}@refs/tags/v0.8.0`));
    assert.ok(regex.test(`${base}@refs/heads/main`));
    for (const bad of [`${base}@refs/heads/feature`, `${base}@refs/tags/v1.2`, `https://github.com/${REPO}/.github/workflows/other.yml@refs/heads/main`, `https://githubXcom/${REPO}/.github/workflows/release.yml@refs/heads/main`, `${base}@refs/heads/main-evil`]) {
        assert.ok(!regex.test(bad), bad);
    }
    assert.ok(!new RegExp(certIdentityRegex(REPO)).test(`https://github.com/AndersonTavares0/LearnVault2/.github/workflows/release.yml@refs/heads/main`));
    assert.throws(() => certIdentityRegex('bad repo'));
});

test('recorded aliases must be explicit: absent is null, never a missing key', () => {
    const aliases = ['latest', 'stable', '0.8', '0.9'];
    assert.deepEqual(validateRecorded(REAL_RECORD, aliases), REAL_RECORD);
    assert.throws(() => validateRecorded({ latest: GOOD, stable: null, '0.8': null }, aliases), /missing: \[0\.9\]/);
    assert.throws(() => validateRecorded({ ...REAL_RECORD, extra: null }, aliases), /unexpected: \[extra\]/);
    assert.throws(() => validateRecorded({ ...REAL_RECORD, stable: 'latest' }, aliases), /neither null nor a sha256/);
    assert.throws(() => validateRecorded([], aliases));
});

test('alias plan over synthetic prior states', () => {
    const cases = [
        ['all present', { latest: GOOD, stable: GOOD, '0.8': GOOD, '0.9': BAD }, { latest: BAD, stable: BAD, '0.8': GOOD, '0.9': BAD },
            { latest: 'repoint', stable: 'repoint', '0.8': 'keep', '0.9': 'keep' }],
        ['minor absent', { latest: GOOD, stable: GOOD, '0.8': GOOD, '0.9': null }, { latest: BAD, stable: BAD, '0.8': GOOD, '0.9': BAD },
            { latest: 'repoint', stable: 'repoint', '0.8': 'keep', '0.9': 'delete' }],
        ['stable absent', { latest: GOOD, stable: null, '0.8': GOOD, '0.9': BAD }, { latest: BAD, stable: BAD, '0.8': GOOD, '0.9': BAD },
            { latest: 'repoint', stable: 'delete', '0.8': 'keep', '0.9': 'keep' }],
        ['only amd64 real v0.8.0 state', REAL_RECORD, { latest: BAD, stable: BAD, '0.8': null, '0.9': BAD },
            { latest: 'repoint', stable: 'delete', '0.8': 'keep', '0.9': 'delete' }],
        ['already restored is a no-op', REAL_RECORD, { latest: GOOD, stable: null, '0.8': null, '0.9': null },
            { latest: 'keep', stable: 'keep', '0.8': 'keep', '0.9': 'keep' }],
    ];
    for (const [name, recorded, current, expected] of cases) {
        assert.deepEqual(Object.fromEntries(planAliases(recorded, current).map(a => [a.alias, a.action])), expected, name);
    }
});

test('staging cleanup only for exactly [staging-<run>]', () => {
    assert.ok(canDeleteStaging(['staging-123'], '123'));
    assert.ok(!canDeleteStaging(['staging-123', '0.9.5'], '123'));
    assert.ok(!canDeleteStaging(['0.9.5', 'staging-123', 'latest'], '123'));
    assert.ok(!canDeleteStaging(['staging-124'], '123'));
    assert.ok(!canDeleteStaging([], '123'));
    assert.ok(!canDeleteStaging(['staging-abc'], 'abc'));
});

test('dry run plan on the real v0.8.0 state mutates nothing and accepts the amd64-only target', () => {
    const { run, state } = backend({ tags: REAL });
    const plan = buildPlan(run, planEnv(REAL_RECORD));
    assert.deepEqual(Object.keys(plan.platforms), ['linux/amd64']);
    assert.equal(plan.targetDigest, GOOD);
    assert.deepEqual(plan.actions.map(a => `${a.alias}:${a.action}`), ['latest:repoint', 'stable:delete', '0.8:keep', '0.9:delete']);
    assert.match(formatPlan(plan), /stable: .* -> absent \(delete\)/);
    assert.deepEqual(mutations(state), []);
    assert.equal(state.tags.stable, BAD);
});

test('apply restores the recorded state, deletes only absent aliases and fixes the GitHub flags', () => {
    const { run, state } = backend({ tags: REAL });
    const plan = buildPlan(run, planEnv(REAL_RECORD));
    applyPlan(run, plan, () => {});
    assert.equal(state.tags.latest, GOOD);
    assert.ok(!('stable' in state.tags) && !('0.9' in state.tags) && !('0.8' in state.tags));
    // The released images keep their tags.
    assert.equal(state.tags['0.9.5'], BAD);
    assert.equal(state.tags['0.8.0'], GOOD);
    assert.equal(state.releases['v0.9.5'].isPrerelease, true);
    assert.equal(state.releases['v0.8.0'].isPrerelease, false);
    assert.equal(state.latest, 'v0.8.0');
    assert.ok(!state.log.some(entry => entry[0] === 'git'));
});

test('deleting an absent alias never deletes a version that carries a release tag', () => {
    const { run, state } = backend({ tags: REAL });
    applyPlan(run, buildPlan(run, planEnv(REAL_RECORD)), () => {});
    const deleted = state.log.filter(entry => entry[0] === 'DELETED').map(entry => entry[1]);
    assert.ok(!deleted.includes(BAD) && !deleted.includes(GOOD));
    assert.equal(deleted.length, 2);
});

test('unrelated prior state: stable present, minor absent', () => {
    const tags = { 'latest': BAD, 'stable': BAD, '0.9.5': BAD, '0.8.0': GOOD, '0.8': GOOD };
    const { run, state } = backend({ tags });
    const recorded = { latest: GOOD, stable: GOOD, '0.8': GOOD, '0.9': null };
    applyPlan(run, buildPlan(run, planEnv(recorded)), () => {});
    assert.deepEqual([state.tags.latest, state.tags.stable, state.tags['0.8'], '0.9' in state.tags], [GOOD, GOOD, GOOD, false]);
});

test('fail closed: unsigned target, uncertain lookups, bad releases, drift', () => {
    assert.throws(() => buildPlan(backend({ tags: REAL, signed: [BAD] }).run, planEnv(REAL_RECORD)), /no signatures found/);
    assert.throws(() => buildPlan(backend({ tags: REAL, releases: { 'v0.8.0': { isPrerelease: true }, 'v0.9.5': { isPrerelease: false } } }).run, planEnv(REAL_RECORD)), /pre-release/);
    assert.throws(() => buildPlan(backend({ tags: REAL, draft: true }).run, planEnv(REAL_RECORD)), /published release/);
    assert.throws(() => buildPlan(backend({ tags: { ...REAL, '0.8.0': undefined } }).run, planEnv(REAL_RECORD)), /does not exist/);
    assert.throws(() => buildPlan(backend({ tags: REAL }).run, planEnv(REAL_RECORD, { badTag: 'v0.8.0' })), /must differ/);
    assert.throws(() => buildPlan(backend({ tags: REAL }).run, planEnv(REAL_RECORD, { stagingRun: 'abc' })), /numeric/);
    assert.throws(() => buildPlan(backend({ tags: REAL }).run, planEnv({ ...REAL_RECORD, stable: GOOD, latest: d('c') })), /not found|resolve|Could not/);
    // A registry error that is not "not found" must not be read as an absent alias.
    const flaky = backend({ tags: REAL });
    const original = flaky.run;
    const run = (command, args) => (args.some(arg => String(arg).endsWith(':stable')) ? { status: 1, stdout: '', stderr: 'i/o timeout' } : original(command, args));
    assert.throws(() => buildPlan(run, planEnv(REAL_RECORD)), /Could not inspect .*:stable/);
    // Registry drift between plan and apply is refused before any mutation.
    const drift = backend({ tags: REAL });
    const plan = buildPlan(drift.run, planEnv(REAL_RECORD));
    drift.state.tags.stable = GOOD;
    assert.throws(() => applyPlan(drift.run, plan, () => {}), /changed since the plan/);
    assert.deepEqual(mutations(drift.state), []);
});

test('apply stops before GitHub flags if aliases cannot be restored exactly', () => {
    const { run, state } = backend({ tags: REAL });
    const plan = buildPlan(run, planEnv(REAL_RECORD));
    // The tombstone keeps the same digest, so the alias could not be detached.
    const stuck = (command, args) => (command === 'docker' && args[2] === 'create' && args.includes('--annotation') ? { status: 0, stdout: '', stderr: '' } : run(command, args));
    assert.throws(() => applyPlan(stuck, plan, () => {}), /could not be detached/);
    assert.equal(state.releases['v0.9.5'].isPrerelease, false);
    assert.equal(state.latest, 'v0.9.5');
});

test('staging cleanup deletes a lone staging tag and skips one that shares a digest with release tags', () => {
    const stagingDigest = d('e');
    const alone = backend({ tags: { ...REAL, 'staging-777': stagingDigest } });
    alone.state.raw[stagingDigest] = indexOf([['amd64', AMD_GOOD]]);
    applyPlan(alone.run, buildPlan(alone.run, planEnv(REAL_RECORD, { stagingRun: '777' })), () => {});
    assert.ok(!('staging-777' in alone.state.tags));

    const shared = backend({ tags: { ...REAL, 'staging-777': BAD } });
    shared.state.tags['0.9.5'] = BAD;
    const record = { latest: GOOD, stable: GOOD, '0.8': GOOD, '0.9': BAD };
    shared.state.tags.stable = GOOD;
    shared.state.tags['0.8'] = GOOD;
    applyPlan(shared.run, buildPlan(shared.run, planEnv(record, { stagingRun: '777' })), () => {});
    assert.equal(shared.state.tags['staging-777'], BAD);
    assert.equal(shared.state.tags['0.9.5'], BAD);
    assert.ok(!shared.state.log.some(entry => entry[0] === 'DELETED'));
});

test('workflow: dry_run defaults to true, own non-cancelling group, Release keeps its per-tag group, never touches git tags', () => {
    assert.match(rollbackYml, /dry_run:[\s\S]*?default: true\n\s+type: boolean/);
    const group = text => /concurrency:\n  group: (.+)\n  cancel-in-progress: (\w+)/.exec(text).slice(1);
    assert.deepEqual(group(rollbackYml), ['rollback', 'false']);
    assert.match(rollbackYml, /allow_unsigned_legacy:[\s\S]*?default: false\n\s+type: boolean/);
    assert.equal(group(releaseYml)[0], 'release-${{ github.event.release.tag_name || inputs.tag }}');
    assert.equal(group(releaseYml)[1], 'false');
    assert.match(rollbackYml, /apply:\n[\s\S]*?if: \$\{\{ !inputs\.dry_run \}\}[\s\S]*?environment: release/);
    assert.doesNotMatch(rollbackYml.split('\n').filter(line => !line.trim().startsWith('#')).join('\n'), /git (tag|push)|--delete-tag|--cleanup-tag|gh release delete/);
    assert.match(rollbackYml, /refs\/heads\/main/);
    // Mutating permissions only in the approved apply job.
    assert.equal((rollbackYml.match(/packages: write/g) ?? []).length, 1);
});

const LEGACY_RECORD = { ...REAL_RECORD, '0.8.0': GOOD };
const legacyRun = (opts = {}) => backend({ tags: REAL, signed: [BAD], ...opts });

test('unsigned legacy v0.8.0 is allowed only with the flag and a matching recorded digest', () => {
    assert.ok(isLegacyTag('v0.8.0') && isLegacyTag('v0.7.9') && !isLegacyTag('v0.8.1') && !isLegacyTag('v0.9.0') && !isLegacyTag('v1.0.0'));
    // Without the flag the unsigned target fails closed.
    assert.throws(() => buildPlan(legacyRun().run, planEnv(LEGACY_RECORD)), /cosign could not confirm|no signatures found/);
    // With flag and digest match: planned with a loud warning, platforms still verified, then applied.
    const { run, state } = legacyRun();
    const plan = buildPlan(run, planEnv(LEGACY_RECORD, { allowUnsignedLegacy: true }));
    assert.deepEqual(plan.unsignedDigests, [GOOD]);
    assert.match(formatPlan(plan), /::warning::UNSIGNED LEGACY IMAGE: sha256:a+/);
    assert.deepEqual(plan.recorded, REAL_RECORD);
    applyPlan(run, plan, () => {});
    assert.equal(state.tags.latest, GOOD);
    assert.equal(state.latest, 'v0.8.0');
});

test('unsigned legacy: digest mismatch, missing record, too-new release or uncertain cosign still fail', () => {
    const flag = { allowUnsignedLegacy: true };
    assert.throws(() => buildPlan(legacyRun().run, planEnv({ ...REAL_RECORD, '0.8.0': d('c') }, flag)), /allow_unsigned_legacy needs/);
    assert.throws(() => buildPlan(legacyRun().run, planEnv(REAL_RECORD, flag)), /allow_unsigned_legacy needs/);
    assert.throws(() => buildPlan(legacyRun({ releases: { 'v0.9.5': { isPrerelease: false }, 'v0.10.0': { isPrerelease: false } } }).run, planEnv({ latest: BAD, stable: BAD, '0.9': BAD, '0.10': BAD, '0.9.5': BAD }, { ...flag, targetTag: 'v0.9.5', badTag: 'v0.10.0' })), /allow_unsigned_legacy needs/);
    // Signed by the wrong identity, or any cosign/registry error that is not "no signatures found", is uncertain.
    for (const cosignError of ['Error: no matching signatures: none of the expected identities matched what was in the certificate', 'Error: i/o timeout', 'Error: getting signed payload: 500']) {
        assert.throws(() => buildPlan(legacyRun({ cosignError }).run, planEnv(LEGACY_RECORD, flag)), /cosign could not confirm/);
    }
    // The flag never relaxes a platform check, nor the signature of a different recorded digest.
    const missingChild = legacyRun();
    delete missingChild.state.raw[GOOD];
    assert.throws(() => buildPlan(missingChild.run, planEnv(LEGACY_RECORD, flag)));
    assert.throws(() => buildPlan(legacyRun({ signed: [] }).run, planEnv({ ...LEGACY_RECORD, '0.9': BAD }, flag)), /cosign could not confirm|no signatures found/);
});

test('refuses to plan or apply while a Release run is in progress, queued or waiting, or if the check fails', () => {
    for (const status of ['in_progress', 'queued', 'waiting']) {
        const busy = backend({ tags: REAL, activeRuns: { [status]: [42] } });
        assert.throws(() => assertNoActiveRelease(busy.run, REPO), new RegExp(`${status} \\(42\\)`));
        assert.throws(() => buildPlan(busy.run, planEnv(REAL_RECORD)), /Release workflow run/);
        assert.deepEqual(mutations(busy.state), []);
    }
    assert.throws(() => assertNoActiveRelease(backend({ tags: REAL, activeRuns: { error: true } }).run, REPO), /failed/);
    assert.doesNotThrow(() => assertNoActiveRelease(backend({ tags: REAL }).run, REPO));
    // A release that starts after the plan blocks the apply before any mutation.
    const { run, state } = backend({ tags: REAL });
    const plan = buildPlan(run, planEnv(REAL_RECORD));
    state.activeRuns = null;
    const busy = (command, args) => (command === 'gh' && args[0] === 'run' && args[args.indexOf('--status') + 1] === 'queued'
        ? { status: 0, stdout: '[{"databaseId":7}]', stderr: '' } : run(command, args));
    assert.throws(() => applyPlan(busy, plan, () => {}), /queued/);
    assert.deepEqual(mutations(state), []);
});

test('tombstones are distinct per alias and never collide with another known digest', () => {
    const { run, state } = backend({ tags: REAL });
    applyPlan(run, buildPlan(run, planEnv(REAL_RECORD)), () => {});
    const created = state.log.filter(entry => entry[1] === 'buildx' && entry[3] === 'create' && entry.includes('--annotation'));
    const annotations = created.map(entry => entry[entry.indexOf('--annotation') + 1]);
    assert.equal(new Set(annotations).size, created.length);
    assert.equal(created.length, 2);
});

test('parseTag is strict', () => {
    assert.deepEqual(parseTag('v0.8.0'), { tag: 'v0.8.0', version: '0.8.0', minor: '0.8' });
    for (const bad of ['0.8.0', 'v1.2', 'v01.2.3', 'v1.2.3-rc.1', '', undefined]) assert.throws(() => parseTag(bad));
});
