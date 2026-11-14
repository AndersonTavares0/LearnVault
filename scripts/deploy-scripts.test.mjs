import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');

test('deploy scripts and configs exist and have proper security settings', () => {
    const composePath = resolve(root, 'deploy/compose/compose.yaml');
    assert.ok(existsSync(composePath), 'compose.yaml deve existir');
    const composeContent = readFileSync(composePath, 'utf8');
    
    assert.match(composeContent, /learnvault_net/, 'deve definir e usar rede dedicada');
    assert.match(composeContent, /json-file/, 'deve definir rotação de logs json-file');
    assert.match(composeContent, /read_only:\s*true/, 'deve manter read_only ativo');
    assert.match(composeContent, /no-new-privileges:\s*true/, 'deve manter no-new-privileges ativo');
    assert.match(composeContent, /healthcheck:/, 'deve ter healthcheck configurado');

    const caddyfilePath = resolve(root, 'deploy/compose/Caddyfile');
    assert.ok(existsSync(caddyfilePath), 'Caddyfile deve existir');
    const caddyContent = readFileSync(caddyfilePath, 'utf8');
    assert.match(caddyContent, /Strict-Transport-Security/, 'deve conter header HSTS');
    assert.match(caddyContent, /X-Content-Type-Options/, 'deve conter header X-Content-Type-Options');
    assert.match(caddyContent, /reverse_proxy\s+learnvault:8090/, 'deve fazer proxy para o container learnvault');

    const updateScript = resolve(root, 'deploy/scripts/update.sh');
    assert.ok(existsSync(updateScript), 'update.sh deve existir');
    const updateContent = readFileSync(updateScript, 'utf8');
    assert.match(updateContent, /backup/, 'update.sh deve criar backup preventivo');
    assert.match(updateContent, /restore/, 'update.sh deve ter suporte a rollback com restore');
    assert.match(updateContent, /update-history\.json/, 'update.sh deve registrar histórico de atualizações');

    const backupScript = resolve(root, 'deploy/scripts/backup.sh');
    assert.ok(existsSync(backupScript), 'backup.sh deve existir');

    const restoreScript = resolve(root, 'deploy/scripts/restore.sh');
    assert.ok(existsSync(restoreScript), 'restore.sh deve existir');

    const daemonScript = resolve(root, 'deploy/scripts/updater-daemon.sh');
    assert.ok(existsSync(daemonScript), 'updater-daemon.sh deve existir');

    const updaterComposePath = resolve(root, 'deploy/compose/compose.updater.yaml');
    assert.ok(existsSync(updaterComposePath), 'compose.updater.yaml deve existir');
    const updaterComposeContent = readFileSync(updaterComposePath, 'utf8');
    assert.match(updaterComposeContent, /image:\s*docker:cli/, 'updater deve usar imagem docker:cli');
    assert.match(updaterComposeContent, /working_dir:\s*\/compose/, 'updater deve ter working_dir /compose');
    assert.match(updaterComposeContent, /docker-proxy:/, 'updater deve usar o proxy do Docker');
    const proxyService = updaterComposeContent.match(/^  docker-proxy:\n[\s\S]*?(?=^  updater:)/m)?.[0] ?? '';
    const updaterService = updaterComposeContent.match(/^  updater:\n[\s\S]*?(?=^networks:)/m)?.[0] ?? '';
    assert.match(proxyService, /- \/var\/run\/docker\.sock:\/var\/run\/docker\.sock:ro/, 'somente o proxy deve montar o socket do Docker');
    assert.match(proxyService, /- \/tmp:rw,noexec,nosuid,size=1m/, 'proxy read-only deve ter tmpfs para o arquivo temporário do HAProxy');
    assert.match(updaterService, /DOCKER_HOST:\s*tcp:\/\/docker-proxy:2375/, 'updater deve apontar para o proxy');
    assert.doesNotMatch(updaterService, /docker\.sock/, 'updater não deve montar o socket do Docker');
    assert.match(updaterComposeContent, /updater_net:\n\s+internal:\s+true/, 'rede do proxy deve ser interna');

    const envExamplePath = resolve(root, 'deploy/compose/.env.example');
    assert.ok(existsSync(envExamplePath), '.env.example deve existir');
    const envExampleContent = readFileSync(envExamplePath, 'utf8');
    assert.match(envExampleContent, /ghcr\.io\/andersontavares0\/learnvault/, '.env.example deve documentar o repositório oficial no GHCR');
    assert.match(updateContent, /ghcr\.io\/andersontavares0\/learnvault/, 'update.sh deve usar o repositório oficial no GHCR');
});

test('release workflow mandates signed Windows updater assets and latest.json', () => {
    const releaseWfPath = resolve(root, '.github/workflows/release.yml');
    assert.ok(existsSync(releaseWfPath), 'release.yml deve existir');
    const content = readFileSync(releaseWfPath, 'utf8');
    assert.match(content, /TAURI_SIGNING_PRIVATE_KEY/, 'deve exigir a chave de assinatura');
    assert.match(content, /createUpdaterArtifacts\s*=\s*"v1Compatible"/, 'deve habilitar createUpdaterArtifacts v1Compatible no release');
    assert.match(content, /learnvault-windows-\$\{RELEASE_TAG\}\.nsis\.zip/, 'deve incluir o bundle nsis.zip nos esperados');
    assert.match(content, /learnvault-windows-\$\{RELEASE_TAG\}\.nsis\.zip\.sig/, 'deve incluir a assinatura .sig nos esperados');
    assert.match(content, /latest\.json/, 'deve incluir latest.json nos esperados');
    assert.match(content, /windows-x86_64-nsis/, 'manifesto latest.json deve incluir o alvo windows-x86_64-nsis');
    assert.match(content, /must be a published stable release \(not a draft or pre-release\)/, 'reparo manual deve exigir release estável publicada');
    assert.match(content, /isPrerelease <<<"\$meta"\)" != "false"/, 'a tag estável deve esperar release não pré-lançamento');
    assert.match(content, /refs\/heads\/main/, 'reparo manual deve usar a definição confiável de main');
    assert.doesNotMatch(content, /gh release create|--draft=false/, 'reparo não deve criar nem publicar releases');
    assert.match(content, /docker\/setup-buildx-action@[0-9a-f]{40}/, 'setup-buildx-action deve usar commit SHA completo');
    assert.match(content, /docker\/login-action@[0-9a-f]{40}/, 'login-action deve usar commit SHA completo');
    assert.match(content, /docker\/metadata-action@[0-9a-f]{40}/, 'metadata-action deve usar commit SHA completo');
    assert.match(content, /docker\/build-push-action@[0-9a-f]{40}/, 'build-push-action deve usar commit SHA completo');
});
