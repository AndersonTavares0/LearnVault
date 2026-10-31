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
    assert.match(updaterComposeContent, /\/var\/run\/docker\.sock/, 'updater companion deve ter acesso ao socket do docker');
});
