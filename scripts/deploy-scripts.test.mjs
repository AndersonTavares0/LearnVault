import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync, readdirSync, mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path, { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');

function normalizeCopyPath(rawPath) {
    return rawPath.trim().replace(/^\.\/+/, '').replace(/\/+$/, '');
}

export function parseContainerfileStageCopies(containerfileSource) {
    const stages = new Map();
    let currentStage = null;

    for (const rawLine of containerfileSource.split(/\r?\n/)) {
        const line = rawLine.trim();
        if (!line || line.startsWith('#')) continue;

        const fromMatch = /^FROM\s+\S+(?:\s+AS\s+([A-Za-z0-9_-]+))?/i.exec(line);
        if (fromMatch) {
            currentStage = fromMatch[1] ?? null;
            if (currentStage && !stages.has(currentStage)) {
                stages.set(currentStage, []);
            }
            continue;
        }

        if (!currentStage || !/^COPY\s+/i.test(line)) continue;
        const tokens = line.replace(/^COPY\s+/i, '').trim().split(/\s+/);
        const args = tokens.filter((token) => !token.startsWith('--'));
        if (tokens.some((token) => token.startsWith('--from=')) || args.length < 2) continue;

        const destination = normalizeCopyPath(args.at(-1));
        const sources = args.slice(0, -1).map(normalizeCopyPath);

        if (sources.length === 1 && destination === sources[0]) {
            stages.get(currentStage).push(sources[0]);
            continue;
        }

        for (const source of sources) {
            const destPath = destination === '' ? source : `${destination}/${path.posix.basename(source)}`;
            if (destPath === source) {
                stages.get(currentStage).push(source);
            }
        }
    }

    return stages;
}

function isCoveredByCopiedPaths(targetPath, copiedPaths) {
    return copiedPaths.some((copied) => targetPath === copied || targetPath.startsWith(`${copied}/`));
}

function collectFiles(dirPath, predicate) {
    const results = [];
    for (const entry of readdirSync(dirPath, { withFileTypes: true })) {
        if (entry.name === 'node_modules' || entry.name === 'target' || entry.name === 'dist') continue;
        const fullPath = path.join(dirPath, entry.name);
        if (entry.isDirectory()) {
            results.push(...collectFiles(fullPath, predicate));
        } else if (entry.isFile() && predicate(fullPath)) {
            results.push(fullPath);
        }
    }
    return results.sort();
}

function stripComments(source) {
    return source
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/^\s*\/\/.*$/gm, '');
}

function extractRelativeSpecifiers(filePath, source) {
    const clean = stripComments(source);
    const specifiers = [];

    if (filePath.endsWith('.rs')) {
        const rustMacroRe = /\b(?:include_str|include_bytes|include)!\s*\(\s*"([^"]+)"\s*\)/g;
        for (const match of clean.matchAll(rustMacroRe)) {
            if (match[1].startsWith('.')) specifiers.push(match[1]);
        }
        const pathAttrRe = /#\[\s*path\s*=\s*"([^"]+)"\s*\]/g;
        for (const match of clean.matchAll(pathAttrRe)) {
            if (match[1].startsWith('.')) specifiers.push(match[1]);
        }
        return specifiers;
    }

    if (filePath.endsWith('.html')) {
        const htmlRefRe = /\b(?:src|href)\s*=\s*["'](\.[^"']+)["']/g;
        for (const match of clean.matchAll(htmlRefRe)) {
            specifiers.push(match[1]);
        }
        return specifiers;
    }

    if (filePath.endsWith('.css')) {
        const cssImportRe = /@import\s+(?:url\(\s*)?['"](\.[^'"]+)['"]\s*\)?/g;
        for (const match of clean.matchAll(cssImportRe)) {
            specifiers.push(match[1]);
        }
        const cssUrlRe = /\burl\(\s*['"]?(\.[^)'"\s]+)['"]?\s*\)/g;
        for (const match of clean.matchAll(cssUrlRe)) {
            specifiers.push(match[1]);
        }
        return specifiers;
    }

    const importExportRe = /\b(?:import|export)\s+(?:[^'"]*?\s+from\s+)?['"](\.[^'"]+)['"]/g;
    for (const match of clean.matchAll(importExportRe)) {
        specifiers.push(match[1]);
    }
    const dynamicImportRe = /\bimport\s*\(\s*['"](\.[^'"]+)['"]\s*\)/g;
    for (const match of clean.matchAll(dynamicImportRe)) {
        specifiers.push(match[1]);
    }
    const urlImportRe = /\bnew\s+URL\s*\(\s*['"](\.[^'"]+)['"]\s*,\s*import\.meta\.url\s*\)/g;
    for (const match of clean.matchAll(urlImportRe)) {
        specifiers.push(match[1]);
    }
    return specifiers;
}

export function auditContainerfileBuildInputs({
    containerfileSource,
    rootDir = root
} = {}) {
    const stages = parseContainerfileStageCopies(containerfileSource);
    const webCopies = stages.get('web-build') ?? [];
    const rustCopies = stages.get('rust-build') ?? [];
    const errors = [];

    const webFiles = collectFiles(
        path.join(rootDir, 'apps/web-next'),
        (file) =>
            /\.(?:[cm]?[jt]sx?|css|html)$/.test(file) &&
            !/\.(?:test|spec)\.[cm]?[jt]sx?$/.test(file) &&
            path.basename(file) !== 'vite.config.ts' &&
            path.basename(file) !== 'eslint.config.js'
    );

    const rustFiles = [
        ...collectFiles(path.join(rootDir, 'crates/core/src'), (file) => file.endsWith('.rs')),
        ...collectFiles(path.join(rootDir, 'crates/server/src'), (file) => file.endsWith('.rs'))
    ];

    const checkFiles = (files, boundaryPrefix, stageName, copiedPaths) => {
        for (const fullFile of files) {
            const relFile = path.relative(rootDir, fullFile).split(path.sep).join('/');
            const fileDir = path.posix.dirname(relFile);
            const source = readFileSync(fullFile, 'utf8');

            for (const rawSpec of extractRelativeSpecifiers(relFile, source)) {
                const cleanSpec = rawSpec.split('?')[0].split('#')[0];
                if (!cleanSpec) continue;
                const targetPath = path.posix.normalize(path.posix.join(fileDir, cleanSpec));
                if (targetPath === boundaryPrefix || targetPath.startsWith(`${boundaryPrefix}/`)) {
                    continue;
                }
                if (targetPath.startsWith('../')) {
                    errors.push(`${relFile} imports "${rawSpec}" outside the repository root (${targetPath})`);
                    continue;
                }
                if (!isCoveredByCopiedPaths(targetPath, copiedPaths)) {
                    errors.push(
                        `${relFile} imports "${rawSpec}" (resolved to "${targetPath}"), which escapes "${boundaryPrefix}/" and is not copied into Containerfile stage "${stageName}"`
                    );
                }
            }
        }
    };

    checkFiles(webFiles, 'apps/web-next', 'web-build', webCopies);
    checkFiles(rustFiles, 'crates', 'rust-build', rustCopies);

    return errors;
}

test('deploy scripts and configs exist and have proper security settings', () => {
    const composePath = resolve(root, 'deploy/compose/compose.yaml');
    assert.ok(existsSync(composePath), 'compose.yaml deve existir');
    const composeContent = readFileSync(composePath, 'utf8');
    
    assert.match(composeContent, /learnvault_net/, 'must define and use a dedicated network');
    assert.match(composeContent, /json-file/, 'must define json-file log rotation');
    assert.match(composeContent, /read_only:\s*true/, 'must keep read_only on');
    assert.match(composeContent, /no-new-privileges:\s*true/, 'must keep no-new-privileges on');
    assert.match(composeContent, /healthcheck:/, 'must have a healthcheck configured');

    const caddyfilePath = resolve(root, 'deploy/compose/Caddyfile');
    assert.ok(existsSync(caddyfilePath), 'Caddyfile deve existir');
    const caddyContent = readFileSync(caddyfilePath, 'utf8');
    assert.match(caddyContent, /Strict-Transport-Security/, 'must contain the HSTS header');
    assert.match(caddyContent, /X-Content-Type-Options/, 'must contain the X-Content-Type-Options header');
    assert.match(caddyContent, /reverse_proxy\s+learnvault:8090/, 'must proxy to the learnvault container');

    const updateScript = resolve(root, 'deploy/scripts/update.sh');
    assert.ok(existsSync(updateScript), 'update.sh deve existir');
    const updateContent = readFileSync(updateScript, 'utf8');
    assert.match(updateContent, /backup/, 'update.sh must take a preventive backup');
    assert.match(updateContent, /restore/, 'update.sh must support rollback with restore');
    assert.match(updateContent, /update-history\.json/, 'update.sh must record the update history');

    const backupScript = resolve(root, 'deploy/scripts/backup.sh');
    assert.ok(existsSync(backupScript), 'backup.sh deve existir');

    const restoreScript = resolve(root, 'deploy/scripts/restore.sh');
    assert.ok(existsSync(restoreScript), 'restore.sh deve existir');

    const daemonScript = resolve(root, 'deploy/scripts/updater-daemon.sh');
    assert.ok(existsSync(daemonScript), 'updater-daemon.sh deve existir');

    const updaterComposePath = resolve(root, 'deploy/compose/compose.updater.yaml');
    assert.ok(existsSync(updaterComposePath), 'compose.updater.yaml deve existir');
    const updaterComposeContent = readFileSync(updaterComposePath, 'utf8');
    assert.match(updaterComposeContent, /image:\s*docker:cli/, 'updater must use the docker:cli image');
    assert.match(updaterComposeContent, /working_dir:\s*\/compose/, 'updater must have working_dir /compose');
    assert.match(updaterComposeContent, /docker-proxy:/, 'updater must use the Docker proxy');
    const proxyService = updaterComposeContent.match(/^  docker-proxy:\n[\s\S]*?(?=^  updater:)/m)?.[0] ?? '';
    const updaterService = updaterComposeContent.match(/^  updater:\n[\s\S]*?(?=^networks:)/m)?.[0] ?? '';
    assert.match(proxyService, /- \/var\/run\/docker\.sock:\/var\/run\/docker\.sock:ro/, 'only the proxy may mount the Docker socket');
    assert.match(proxyService, /- \/tmp:rw,noexec,nosuid,size=1m/, 'a read-only proxy must have a tmpfs for HAProxy temporary files');
    assert.match(updaterService, /DOCKER_HOST:\s*tcp:\/\/docker-proxy:2375/, 'updater must point at the proxy');
    assert.doesNotMatch(updaterService, /docker\.sock/, 'updater must not mount the Docker socket');
    assert.match(updaterComposeContent, /updater_net:\n\s+internal:\s+true/, 'the proxy network must be internal');

    const envExamplePath = resolve(root, 'deploy/compose/.env.example');
    assert.ok(existsSync(envExamplePath), '.env.example deve existir');
    const envExampleContent = readFileSync(envExamplePath, 'utf8');
    assert.match(envExampleContent, /ghcr\.io\/andersontavares0\/learnvault/, '.env.example must document the official GHCR repository');
    assert.match(updateContent, /ghcr\.io\/andersontavares0\/learnvault/, 'update.sh must use the official GHCR repository');
});

test('release workflow mandates signed Windows updater assets and latest.json', () => {
    const releaseWfPath = resolve(root, '.github/workflows/release.yml');
    assert.ok(existsSync(releaseWfPath), 'release.yml deve existir');
    const content = readFileSync(releaseWfPath, 'utf8');
    assert.match(content, /TAURI_SIGNING_PRIVATE_KEY/, 'must require the signing key');
    assert.match(content, /createUpdaterArtifacts\s*=\s*"v1Compatible"/, 'must enable createUpdaterArtifacts v1Compatible in the release');
    assert.match(content, /learnvault-windows-\$\{RELEASE_TAG\}\.nsis\.zip/, 'must include the nsis.zip bundle in the expected assets');
    assert.match(content, /learnvault-windows-\$\{RELEASE_TAG\}\.nsis\.zip\.sig/, 'must include the .sig signature in the expected assets');
    assert.match(content, /latest\.json/, 'must include latest.json in the expected assets');
    assert.match(content, /windows-x86_64-nsis/, 'the latest.json manifest must include the windows-x86_64-nsis target');
    assert.match(content, /must be a published release \(not a draft\)/, 'a manual repair must require a published release');
    assert.match(content, /prerelease=\$prerelease/, 'the workflow must expose whether the release is a pre-release');
    assert.match(content, /refs\/heads\/main/, 'a manual repair must use the trusted definition of main');
    assert.doesNotMatch(content, /gh release create|--draft=false/, 'a repair must not create or publish releases');
    assert.match(content, /docker\/setup-buildx-action@[0-9a-f]{40}/, 'setup-buildx-action must use a full commit SHA');
    assert.match(content, /docker\/login-action@[0-9a-f]{40}/, 'login-action must use a full commit SHA');
    assert.match(content, /docker\/metadata-action@[0-9a-f]{40}/, 'metadata-action must use a full commit SHA');
    assert.match(content, /docker\/build-push-action@[0-9a-f]{40}/, 'build-push-action must use a full commit SHA');
});

test('Containerfile copies every repository file referenced by relative imports escaping apps/web-next or crates', () => {
    const containerfilePath = resolve(root, 'deploy/container/Containerfile');
    assert.ok(existsSync(containerfilePath), 'Containerfile must exist');
    const containerfileSource = readFileSync(containerfilePath, 'utf8');

    assert.deepEqual(
        auditContainerfileBuildInputs({ containerfileSource, rootDir: root }),
        [],
        'Every relative import or compile-time include escaping apps/web-next/ or crates/ must be copied into the corresponding Containerfile stage'
    );

    const fixtureRoot = mkdtempSync(path.join(tmpdir(), 'lv-container-audit-'));
    try {
        mkdirSync(path.join(fixtureRoot, 'apps/web-next/src/pages/settings'), { recursive: true });
        mkdirSync(path.join(fixtureRoot, 'crates/core/src'), { recursive: true });
        mkdirSync(path.join(fixtureRoot, 'crates/server/src'), { recursive: true });
        writeFileSync(
            path.join(fixtureRoot, 'apps/web-next/src/pages/settings/about.tsx'),
            [
                "import lightSeal from '../../../../../docs/design/branding/learnvault-seal.svg';",
                "import darkSeal from '../../../../../docs/design/branding/learnvault-seal-dark.svg';"
            ].join('\n')
        );

        assert.deepEqual(
            auditContainerfileBuildInputs({ containerfileSource, rootDir: fixtureRoot }),
            []
        );

        const preFixContainerfile = containerfileSource
            .split(/\r?\n/)
            .filter((line) => !line.includes('docs/design/branding'))
            .join('\n');
        const regressions = auditContainerfileBuildInputs({
            containerfileSource: preFixContainerfile,
            rootDir: fixtureRoot
        });

        assert.equal(regressions.length, 2, 'Removing COPY docs/design/branding must flag both light and dark seal imports');
        assert.match(regressions[0], /apps\/web-next\/src\/pages\/settings\/about\.tsx/);
        assert.match(regressions[0], /docs\/design\/branding\/learnvault-seal\.svg/);
        assert.match(regressions[1], /docs\/design\/branding\/learnvault-seal-dark\.svg/);
    } finally {
        rmSync(fixtureRoot, { recursive: true, force: true });
    }
});
