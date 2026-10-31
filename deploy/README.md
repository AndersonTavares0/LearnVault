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
- `LIBRARY_DIR`: Caminho da pasta que contém os cursos no seu computador/servidor.

### 2. Executar com Docker Compose

```sh
docker compose up -d
```

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
   ```
2. Inicie o container companion:
   ```sh
   docker compose -f compose.yaml -f compose.updater.yaml up -d
   ```
O container companion updater roda isoladamente, consulta a API e só aplica atualizações dentro da janela de manutenção definida.
