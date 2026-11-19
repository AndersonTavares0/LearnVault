#!/usr/bin/env node
// Rollback planner/executor for GHCR moving aliases and GitHub release flags.
// Invoked by .github/workflows/rollback.yml. Every external call goes through an injected
// `run(command, args)` so tests can drive it with a synthetic registry. Arguments are passed as
// an argv array (never through a shell), so regular expressions need exactly one level of escaping.
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const STRICT_TAG = /^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const OIDC_ISSUER = 'https://token.actions.githubusercontent.com';
const ABSENT = /not found|manifest unknown|name unknown|MANIFEST_UNKNOWN|NAME_UNKNOWN/i;

export function parseTag(tag) {
    const match = STRICT_TAG.exec(tag ?? '');
    if (!match) throw new Error(`Unexpected release tag format: ${tag}`);
    const version = tag.slice(1);
    return { tag, version, minor: `${match[1]}.${match[2]}` };
}

// The signing identity keeps the repository's original case; only the image name is lowercase.
export function certIdentityRegex(repository) {
    if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error(`Unexpected repository: ${repository}`);
    const escaped = repository.replaceAll('.', '\\.');
    return `^https://github\\.com/${escaped}/\\.github/workflows/release\\.yml@refs/(tags/v[0-9]+\\.[0-9]+\\.[0-9]+|heads/main)$`;
}

// Real image platforms only: attestation manifests are reported as unknown/unknown.
export function platformsOf(rawIndex) {
    const index = typeof rawIndex === 'string' ? JSON.parse(rawIndex) : rawIndex;
    if (!Array.isArray(index?.manifests)) throw new Error('Not an image index');
    const platforms = new Map();
    for (const manifest of index.manifests) {
        const { os, architecture } = manifest.platform ?? {};
        if (!os || !architecture || os === 'unknown' || architecture === 'unknown') continue;
        if (!DIGEST.test(manifest.digest ?? '')) throw new Error('Index entry without a valid digest');
        platforms.set(`${os}/${architecture}`, manifest.digest);
    }
    if (platforms.size === 0) throw new Error('Image index has no runnable platform');
    return Object.fromEntries([...platforms].sort(([a], [b]) => a.localeCompare(b)));
}

export function aliasNames(targetTag, badTag) {
    return [...new Set(['latest', 'stable', parseTag(targetTag).minor, parseTag(badTag).minor])];
}

// `recorded` must name every alias explicitly: a digest, or null for "did not exist".
// A missing key is rejected so a typo can never be read as "absent".
export function validateRecorded(recorded, aliases, optionalKey) {
    if (!recorded || typeof recorded !== 'object' || Array.isArray(recorded)) throw new Error('Recorded aliases must be a JSON object');
    const allowed = optionalKey ? [...aliases, optionalKey] : aliases;
    const extra = Object.keys(recorded).filter(key => !allowed.includes(key));
    const missing = aliases.filter(alias => !(alias in recorded));
    if (extra.length || missing.length) {
        throw new Error(`Recorded aliases must be exactly [${aliases.join(', ')}] (missing: [${missing}], unexpected: [${extra}])`);
    }
    for (const alias of allowed.filter(key => key in recorded)) {
        const value = recorded[alias];
        if (value !== null && !DIGEST.test(value)) throw new Error(`Recorded alias ${alias} is neither null nor a sha256 digest`);
    }
    return Object.fromEntries(allowed.filter(key => key in recorded).map(alias => [alias, recorded[alias]]));
}

// Images published before keyless signing existed (v0.8.0 and older) carry no signature at all.
export const isLegacyTag = tag => {
    const [major, minor, patch] = parseTag(tag).version.split('.').map(Number);
    return major === 0 && (minor < 8 || (minor === 8 && patch === 0));
};
const UNSIGNED = /no signatures found/i;

// Never mutate while a Release run could be moving the same aliases. A failed lookup counts as busy.
export function assertNoActiveRelease(run, repository) {
    for (const status of ['in_progress', 'queued', 'waiting']) {
        const out = sh(run, 'gh', ['run', 'list', '--repo', repository, '--workflow', 'release.yml', '--status', status, '--json', 'databaseId', '--limit', '100']).stdout;
        let runs;
        try { runs = JSON.parse(out); } catch { throw new Error('Could not read the Release workflow runs'); }
        if (!Array.isArray(runs)) throw new Error('Could not read the Release workflow runs');
        if (runs.length) throw new Error(`A Release workflow run is ${status} (${runs.map(r => r.databaseId).join(', ')}); wait for it to finish`);
    }
}

// Compare the live registry with the recorded state, alias by alias.
export function planAliases(recorded, current) {
    return Object.keys(recorded).map(alias => {
        const want = recorded[alias];
        const have = current[alias] ?? null;
        if (want === have) return { alias, action: 'keep', digest: want };
        if (want === null) return { alias, action: 'delete', from: have };
        return { alias, action: 'repoint', digest: want, from: have };
    });
}

// A GHCR version can only be deleted whole, with every tag it carries.
export function canDeleteExactly(tags, expected) {
    return Array.isArray(tags) && tags.length === 1 && tags[0] === expected;
}
export const canDeleteStaging = (tags, runId) => /^[0-9]+$/.test(String(runId)) && canDeleteExactly(tags, `staging-${runId}`);

function sh(run, command, args, { allowFail = false } = {}) {
    const result = run(command, args);
    if (result.status !== 0 && !allowFail) {
        throw new Error(`${command} ${args.slice(0, 3).join(' ')} failed: ${(result.stderr || result.stdout || '').trim()}`);
    }
    return result;
}

export function inspectDigest(run, ref) {
    const result = sh(run, 'docker', ['buildx', 'imagetools', 'inspect', ref, '--format', '{{.Manifest.Digest}}'], { allowFail: true });
    if (result.status === 0) {
        const digest = result.stdout.trim();
        if (!DIGEST.test(digest)) throw new Error(`Could not read a digest for ${ref}`);
        return digest;
    }
    if (ABSENT.test(`${result.stderr}${result.stdout}`)) return null;
    throw new Error(`Could not inspect ${ref}: ${(result.stderr || result.stdout).trim()}`); // uncertain: fail closed
}

export function snapshotAliases(run, image, aliases) {
    return Object.fromEntries(aliases.map(alias => [alias, inspectDigest(run, `${image}:${alias}`)]));
}

// Each platform present in the index must resolve, and the index must carry a valid signature.
// `allowUnsigned` skips ONLY the "no signatures found" outcome; a wrong identity or any other
// cosign/registry error still fails. Returns { platforms, unsigned }.
export function verifyDigest(run, image, digest, repository, { allowUnsigned = false } = {}) {
    const raw = sh(run, 'docker', ['buildx', 'imagetools', 'inspect', '--raw', `${image}@${digest}`]).stdout;
    const platforms = platformsOf(raw);
    for (const [platform, child] of Object.entries(platforms)) {
        if (inspectDigest(run, `${image}@${child}`) !== child) throw new Error(`Platform ${platform} of ${digest} does not resolve`);
    }
    const result = sh(run, 'cosign', [
        'verify', `${image}@${digest}`,
        '--certificate-oidc-issuer', OIDC_ISSUER,
        '--certificate-identity-regexp', certIdentityRegex(repository),
    ], { allowFail: allowUnsigned });
    if (result.status === 0) return { platforms, unsigned: false };
    const text = `${result.stderr}${result.stdout}`;
    if (UNSIGNED.test(text) && !/expected identities|certificate/i.test(text)) return { platforms, unsigned: true };
    throw new Error(`cosign could not confirm ${digest}: ${text.trim()}`);
}

function releaseMeta(run, repository, tag) {
    const meta = JSON.parse(sh(run, 'gh', ['release', 'view', tag, '--repo', repository, '--json', 'tagName,isDraft,isPrerelease']).stdout);
    if (meta.tagName !== tag || meta.isDraft !== false || typeof meta.isPrerelease !== 'boolean') {
        throw new Error(`${tag} must be a published release`);
    }
    return meta;
}

export function buildPlan(run, env) {
    const { repository, targetTag, badTag, stagingRun, allowUnsignedLegacy = false } = env;
    const target = parseTag(targetTag);
    const bad = parseTag(badTag);
    if (target.tag === bad.tag) throw new Error('The known-good and bad releases must differ');
    if (stagingRun && !/^[0-9]+$/.test(stagingRun)) throw new Error('staging_run must be a numeric workflow run id');
    const image = `ghcr.io/${repository.toLowerCase()}`;
    const aliases = aliasNames(target.tag, bad.tag);
    const recorded = validateRecorded(env.recorded, aliases, target.version);
    assertNoActiveRelease(run, repository);

    if (releaseMeta(run, repository, target.tag).isPrerelease) throw new Error(`${target.tag} is a pre-release and cannot be the known-good release`);
    releaseMeta(run, repository, bad.tag);
    const currentLatest = sh(run, 'gh', ['api', `repos/${repository}/releases/latest`, '--jq', '.tag_name']).stdout.trim();

    const targetDigest = inspectDigest(run, `${image}:${target.version}`);
    if (!targetDigest) throw new Error(`${image}:${target.version} does not exist`);
    // Unsigned legacy: explicit flag, a release <= v0.8.0, and the live digest must equal the digest the
    // operator recorded for that exact version. Only that digest may skip the signature.
    const legacyOk = allowUnsignedLegacy && isLegacyTag(target.tag) && recorded[target.version] === targetDigest;
    if (allowUnsignedLegacy && !legacyOk) {
        throw new Error(`allow_unsigned_legacy needs a release <= v0.8.0 and recorded_aliases["${target.version}"] equal to the live digest ${targetDigest}`);
    }
    const verified = {};
    const unsignedDigests = [];
    for (const digest of new Set([targetDigest, ...Object.values(recorded).filter(Boolean)])) {
        verified[digest] = verifyDigest(run, image, digest, repository, { allowUnsigned: legacyOk && digest === targetDigest });
        if (verified[digest].unsigned) unsignedDigests.push(digest);
    }
    const aliasRecord = Object.fromEntries(aliases.map(alias => [alias, recorded[alias]]));
    const snapshot = snapshotAliases(run, image, aliases);
    return {
        repository, image, target, bad, stagingRun: stagingRun || '',
        targetDigest, platforms: verified[targetDigest].platforms, recorded: aliasRecord, snapshot,
        allowUnsignedLegacy: legacyOk, unsignedDigests,
        actions: planAliases(aliasRecord, snapshot),
        github: { currentLatest, markPrerelease: bad.tag, restoreLatest: target.tag },
    };
}

export function formatPlan(plan) {
    const lines = [
        ...(plan.unsignedDigests?.length ? [`::warning::UNSIGNED LEGACY IMAGE: ${plan.unsignedDigests.join(', ')} has no cosign signature. It is accepted only because allow_unsigned_legacy was set, the release is <= v0.8.0 and the digest equals the operator record. Platforms were still verified.`] : []),
        `Known-good release: ${plan.target.tag} (${plan.targetDigest}; ${Object.keys(plan.platforms).join(', ')})`,
        `Bad release: ${plan.bad.tag}; current GitHub latest: ${plan.github.currentLatest}`,
        ...plan.actions.map(a => a.action === 'keep'
            ? `alias ${a.alias}: keep ${a.digest ?? 'absent'}`
            : a.action === 'repoint'
                ? `alias ${a.alias}: ${a.from ?? 'absent'} -> ${a.digest}`
                : `alias ${a.alias}: ${a.from} -> absent (delete)`),
        `GitHub: mark ${plan.github.markPrerelease} as pre-release; mark ${plan.github.restoreLatest} as latest; git tags untouched`,
        plan.stagingRun ? `Staging cleanup: staging-${plan.stagingRun} only if it is the sole tag of its version` : 'Staging cleanup: none',
    ];
    return lines.join('\n');
}

function packageApi(run, plan) {
    const [owner, name] = plan.image.replace('ghcr.io/', '').split('/');
    const type = sh(run, 'gh', ['api', `users/${owner}`, '--jq', '.type']).stdout.trim();
    if (type !== 'User' && type !== 'Organization') throw new Error(`Unexpected owner type: ${type}`);
    return `${type === 'User' ? 'users' : 'orgs'}/${owner}/packages/container/${name}/versions`;
}

function versionsWithTag(run, path, tag) {
    const out = sh(run, 'gh', ['api', '--paginate', path, '--jq', '.[] | {id: .id, tags: .metadata.container.tags}']).stdout;
    return out.split('\n').filter(Boolean).map(line => JSON.parse(line)).filter(version => version.tags.includes(tag));
}

function deleteVersionWithExactTag(run, plan, tag, allowed) {
    const path = packageApi(run, plan);
    const versions = versionsWithTag(run, path, tag);
    if (versions.length === 0) return 'absent';
    if (versions.length !== 1 || !allowed(versions[0].tags)) return 'shared';
    sh(run, 'gh', ['api', '-X', 'DELETE', `${path}/${versions[0].id}`]);
    return 'deleted';
}

export function applyPlan(run, plan, log = console.log) {
    const { image, repository } = plan;
    const aliases = Object.keys(plan.recorded);
    assertNoActiveRelease(run, repository);
    // The approval wait can be long: refuse to act on a registry that changed since the plan.
    const now = snapshotAliases(run, image, aliases);
    if (JSON.stringify(now) !== JSON.stringify(plan.snapshot)) throw new Error('Alias state changed since the plan was made; re-run the dry run');
    if (inspectDigest(run, `${image}:${plan.target.version}`) !== plan.targetDigest) throw new Error('Target digest changed since the plan was made');
    for (const digest of new Set([plan.targetDigest, ...Object.values(plan.recorded).filter(Boolean)])) {
        const { unsigned } = verifyDigest(run, image, digest, repository, { allowUnsigned: plan.allowUnsignedLegacy && digest === plan.targetDigest });
        if (unsigned && !(plan.unsignedDigests ?? []).includes(digest)) throw new Error(`${digest} is unsigned but was not accepted by the plan`);
    }

    for (const step of plan.actions) {
        if (step.action === 'repoint') {
            sh(run, 'docker', ['buildx', 'imagetools', 'create', '--tag', `${image}:${step.alias}`, `${image}@${step.digest}`]);
        } else if (step.action === 'delete') {
            // GHCR cannot untag: move the alias to a tombstone index with its own digest, then delete only that version.
            sh(run, 'docker', ['buildx', 'imagetools', 'create', '--annotation', `index:org.opencontainers.image.description=rollback tombstone ${step.alias}`,
                '--tag', `${image}:${step.alias}`, `${image}@${step.from}`]);
            const tomb = inspectDigest(run, `${image}:${step.alias}`);
            const others = new Set([plan.targetDigest, ...Object.values(plan.snapshot), ...Object.values(plan.recorded)].filter(Boolean));
            if (!tomb || others.has(tomb)) throw new Error(`Alias ${step.alias} could not be detached from ${step.from}`);
            const outcome = deleteVersionWithExactTag(run, plan, step.alias, tags => canDeleteExactly(tags, step.alias));
            if (outcome !== 'deleted') throw new Error(`Refusing to delete alias ${step.alias}: version outcome ${outcome}`);
        }
        log(`done: ${step.alias} (${step.action})`);
    }
    const after = snapshotAliases(run, image, aliases);
    if (JSON.stringify(after) !== JSON.stringify(plan.recorded)) throw new Error(`Alias state after rollback differs from the record: ${JSON.stringify(after)}`);

    sh(run, 'gh', ['release', 'edit', plan.github.markPrerelease, '--repo', repository, '--prerelease']);
    sh(run, 'gh', ['release', 'edit', plan.github.restoreLatest, '--repo', repository, '--latest']);
    const latest = sh(run, 'gh', ['api', `repos/${repository}/releases/latest`, '--jq', '.tag_name']).stdout.trim();
    if (latest !== plan.github.restoreLatest) throw new Error(`GitHub latest is ${latest}, expected ${plan.github.restoreLatest}`);

    if (plan.stagingRun) {
        const tag = `staging-${plan.stagingRun}`;
        const outcome = deleteVersionWithExactTag(run, plan, tag, tags => canDeleteStaging(tags, plan.stagingRun));
        log(`staging cleanup ${tag}: ${outcome}`);
    }
}

export const realRun = (command, args) => {
    const result = spawnSync(command, args, { encoding: 'utf8' });
    return { status: result.status ?? 1, stdout: result.stdout ?? '', stderr: result.stderr ?? (result.error?.message ?? '') };
};

function cli(argv, env) {
    const [mode, file] = argv;
    if (mode === 'plan') {
        let recorded;
        try { recorded = JSON.parse(env.RECORDED_ALIASES ?? ''); } catch { throw new Error('RECORDED_ALIASES is not valid JSON'); }
        const plan = buildPlan(realRun, {
            repository: env.REPOSITORY, targetTag: env.TARGET_TAG, badTag: env.BAD_TAG, stagingRun: env.STAGING_RUN, recorded, allowUnsignedLegacy: env.ALLOW_UNSIGNED_LEGACY === 'true',
        });
        writeFileSync(file, JSON.stringify(plan, null, 2));
        console.log(formatPlan(plan));
    } else if (mode === 'apply') {
        applyPlan(realRun, JSON.parse(readFileSync(file, 'utf8')));
    } else {
        throw new Error('Usage: rollback.mjs plan|apply <plan.json>');
    }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    try { cli(process.argv.slice(2), process.env); } catch (error) { console.error(`::error::${error.message}`); process.exit(1); }
}
