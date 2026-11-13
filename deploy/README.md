# LearnVault - Guia de Deploy e Self-Hosting com Docker

Este diretório contém os artefatos de implantação oficial do **LearnVault** para ambientes de produção e self-hosting com Docker e Podman.

---

## Princípios de Arquitetura

1. **Biblioteca Somente Leitura (`/library:ro`)**: O LearnVault nunca grava, move, renomeia ou exclui seus arquivos de cursos originais.
2. **Segurança por Padrão (Zero Privilégios)**:
   - Execução como usuário não-root (`UID 10001:10001`).
   - `read_only: true` no sistema de arquivos do container (apenas `/tmp` montado como tmpfs estrito).
   - `no-new-privileges: true` e `cap_drop: ALL`.
   - Isolamento em rede interna (`learnvault_net`).
   - Sem montagem do Docker Socket dentro do container da aplicação.
3. **Confiabilidade e Persistência**:
   - SQLite em modo WAL com integridade transacional.
   - Ferramenta nativa de backup e restauração com verificação de integridade (`PRAGMA integrity_check`).
4. **Atualizações Confiáveis**:
   - Detecção de novas versões via GitHub Releases API sem vazar dados locais.
   - Script de atualização com **backup preventivo automático**, teste de healthcheck e **rollback imediato** em caso de falha.

---

## Estrutura de Arquivos

- `compose/compose.yaml`: Arquivo principal do Docker Compose.
- `compose/compose.proxy.yaml`: Reverse proxy Caddy com HTTPS automático e headers OWASP.
- `compose/compose.secrets.yaml`: Configuração opcional via Docker Secrets.
- `compose/compose.updater.yaml`: Serviço companion opcional (opt-in) para atualizações automáticas.
- `compose/Caddyfile`: Configuração de proxy reverso com otimização de streaming HTTP Range.
- `compose/.env.example`: Modelo de variáveis de ambiente.
- `container/Containerfile`: Definição OCI multi-stage (Node.js + Rust Alpine).
- `scripts/backup.sh`: Utilitário de backup transacional.
- `scripts/restore.sh`: Utilitário de restauração segura com validação.
- `scripts/update.sh`: Script de atualização com backup preventivo e rollback.
- `scripts/updater-daemon.sh`: Daemon de atualização automática agendada.

---

## Como Iniciar a Aplicação

### 1. Configuração Inicial

```sh
cd deploy/compose
cp .env.example .env
```

Edite o arquivo `.env` e defina suas credenciais de administrador e caminhos:
- `ADMIN_USERNAME`: Usuário administrador inicial.
- `ADMIN_PASSWORD`: Senha forte (mínimo de 12 caracteres).
- `DATA_DIR`: Onde o banco de dados e metadados serão armazenados.
- `LIBRARY_DIR`: Folder on the host, mounted read-only at `/library`. It sets the root the server is allowed to read; it does not register your course folders by itself. On the first start the folder itself appears as one course (named after it). To list each course folder under it, sign in as the administrator, use **Import course** for each one, then **Refresh courses**. Once your course folders are imported, remove the root course (**Manage course**, then remove) so every file is not indexed twice. See #318.

### 2. Executar com Docker Compose

By default Compose uses the official image published on the GitHub Container Registry (GHCR), so a new release is one `docker compose pull && docker compose up -d` away (or automatic, with the opt-in updater below). You can also pin a version or build the image yourself:

```sh
# Option A: the official image (the default; recommended for production)
docker compose up -d

# Option B: pin a version
LEARNVAULT_IMAGE=ghcr.io/andersontavares0/learnvault:0.9.0-rc.4 docker compose up -d

# Option C: build the image from the source code (contributors)
docker compose -f compose.yaml -f compose.build.yaml up -d --build
```

> **Before 1.0:** releases are published as release candidates only, so a candidate publishes the exact
> tag (`0.9.0-rc.4`) and the moving `rc` channel tag, and never moves `latest`, `stable`, or the minor
> line. Until the first stable release, Option A fails to pull and you must pin `LEARNVAULT_IMAGE` as
> in Option B, or follow the newest candidate with `:rc`.

> **Upgrading from a setup that built the image locally:** if your `.env` still has `LEARNVAULT_IMAGE=learnvault-rust:local`, remove that line (the default is now the published image), or keep building by adding `-f compose.build.yaml` to every `docker compose` command.

Acesse no seu navegador: `http://localhost:8090`.

### 3. Executar com Proxy Reverso Caddy (HTTPS Automático)

Para ambientes expostos na rede local ou na internet com domínio próprio e certificados TLS automáticos:

```sh
DOMAIN=cursos.meudominio.com docker compose -f compose.yaml -f compose.proxy.yaml up -d
```

O Caddy cuidará automaticamente da renovação de certificados SSL/TLS, compressão zstd/gzip e cabeçalhos de proteção (HSTS, CSP, X-Frame-Options, X-Content-Type-Options).

---

## Backup e Restauração

### Criando um Backup Transacional

Para criar um backup a quente do banco de dados sem interromper a reprodução dos usuários:

```sh
# Via utilitário script
../../deploy/scripts/backup.sh

# Ou diretamente pelo CLI do container
docker compose exec learnvault /app/learnvault-server backup --database /data/learnvault.sqlite --output /data/backups/meu-backup.sqlite
```

O comando realiza o backup online via API do SQLite e roda `PRAGMA integrity_check` antes de finalizar.

### Restaurando a Partir de um Backup

```sh
# Via script (com validação prévia de integridade e swap atômico)
../../deploy/scripts/restore.sh /caminho/para/meu-backup.sqlite
```

---

## Sistema de Atualizações

### Atualização Manual com Rollback Seguro

Quando uma nova versão for publicada, você pode executar o script de atualização:

```sh
../../deploy/scripts/update.sh [versao-opcional]
```

O script executa:
1. Consulta da versão atual instalada.
2. Backup pré-atualização transacional.
3. Download da imagem versionada.
4. Reinicialização dos containers.
5. Verificação de `healthcheck` durante 60 segundos.
6. **Rollback automático**: se o novo container falhar no healthcheck ou a migração for incompatível, o banco pré-atualização é restaurado e a imagem anterior volta a rodar.
7. Registro de log em `/data/update-history.json`.

### Atualização Automática Opt-in

Caso deseje que o LearnVault verifique e atualize autonomamente durante a madrugada:
1. No `.env`, defina:
   ```env
   AUTO_UPDATE=true
   AUTO_UPDATE_WINDOW=03:00-05:00
   # stable (padrão) instala apenas versões finais; rc também aceita candidatas.
   LEARNVAULT_CHANNEL=stable
   ```
2. Inicie o container companion:
   ```sh
   docker compose -f compose.yaml -f compose.updater.yaml --profile updater up -d
   ```
O container companion updater roda isoladamente, consulta a API e só aplica atualizações dentro da janela de manutenção definida. Ele nunca monta o socket do Docker: quem fala com o daemon é um proxy que libera apenas os endpoints necessários.

`LEARNVAULT_CHANNEL` limita o que pode ser instalado: `stable` recusa candidatas e `rc` também as aceita. Nos dois casos a imagem precisa de uma assinatura válida do workflow de release, e o updater não rebaixa uma instalação: um alvo igual ou mais antigo é ignorado.

**Docker socket security.** The updater never mounts the host socket: it talks to `docker-proxy` (`tecnativa/docker-socket-proxy`, pinned by digest), which only enables the API sections used by `update.sh` (containers, start/stop, images, networks, volumes and exec) and refuses the rest (secrets, swarm, build, commit and plugins). This reduces the surface; it does not eliminate the risk: anyone able to create containers through the API could, in theory, mount the host. Therefore the updater is opt-in and should run only on machines you control. For a stronger guarantee, prefer the Quadlet with `podman-auto-update.timer`, which does not use a socket.

## Updater failure boundaries

Automatic updates remain opt-in. Maintenance windows use the updater container's local clock, include their start minute, and exclude their end minute. Overnight windows such as `23:30-01:15` are supported. Equal endpoints and malformed clock values are rejected. `CHECK_INTERVAL_SECONDS` must contain at most ten decimal digits and represent a value from 1 to 2147483647; leading zeros are accepted.

An update requires a successful backup and target image download before restarting the service. A healthy container is accepted only when its reported version matches the requested stable version. Startup failure or a version mismatch enters rollback using the previous immutable image ID for database restoration and service restart. A failed restore or restart stops recovery and leaves a nonzero exit status; it is never reported as success.

An atomic lock directory in `DATA_DIR/.update.lock` prevents simultaneous attempts against the same data directory. A process crash can leave a stale lock: after confirming that no updater is running, the operator must remove the empty lock directory before retrying. Do not remove an active updater's lock.
