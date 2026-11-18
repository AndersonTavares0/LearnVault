import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');

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
