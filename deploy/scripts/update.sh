#!/bin/sh
# LearnVault - Script Autônomo de Atualização com Backup e Rollback Seguro
#
# Etapas:
# 1. Identifica a versão atual e a versão alvo
# 2. Executa backup transacional preventivo com verificação de integridade
# 3. Baixa a nova imagem versionada
# 4. Atualiza os containers com Docker Compose
# 5. Valida o healthcheck pós-inicialização e migrações
# 6. Em caso de falha: executa ROLLBACK automático para a imagem e banco anteriores
# 7. Registra histórico estruturado em /data/update-history.json
set -eu

CONTAINER="${LEARNVAULT_CONTAINER:-learnvault}"
DATABASE_PATH="/data/learnvault.sqlite"
DATA_DIR="${DATA_DIR:-/data}"
HISTORY_FILE="${DATA_DIR}/update-history.json"
TARGET_VERSION="${1:-}"

log() {
    printf "[%s] %s\n" "$(date +'%Y-%m-%d %H:%M:%S')" "$*"
}

record_history() {
    status="$1"
    from_ver="$2"
    to_ver="$3"
    msg="$4"
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    mkdir -p "$(dirname "$HISTORY_FILE")"
    entry="{\"timestamp\":\"${timestamp}\",\"status\":\"${status}\",\"from\":\"${from_ver}\",\"to\":\"${to_ver}\",\"message\":\"${msg}\"}"
    
    if [ ! -f "$HISTORY_FILE" ] || [ ! -s "$HISTORY_FILE" ]; then
        printf "[\n  %s\n]\n" "$entry" > "$HISTORY_FILE"
    else
        tmp_history=$(mktemp)
        # Remove a última linha que fecha o array ']'
        sed '$d' "$HISTORY_FILE" > "$tmp_history"
        printf "  ,%s\n]\n" "$entry" >> "$tmp_history"
        mv "$tmp_history" "$HISTORY_FILE"
    fi
}

log "Iniciando verificação de atualização do LearnVault..."

# 1. Consulta versão atual
current_health=$(docker compose exec -T "$CONTAINER" wget -q -O - http://127.0.0.1:8090/api/healthz 2>/dev/null || true)
CURRENT_VERSION=$(printf "%s" "$current_health" | grep -o '"version":"[^"]*"' | cut -d'"' -f4 || echo "desconhecida")

if [ -z "$CURRENT_VERSION" ] || [ "$CURRENT_VERSION" = "desconhecida" ]; then
    log "Não foi possível obter a versão atual do container em execução."
fi

log "Versão atual instalada: ${CURRENT_VERSION}"

# Se não foi fornecida versão alvo, consulta o endpoint interno ou o repositório oficial
if [ -z "$TARGET_VERSION" ]; then
    log "Consultando versão mais recente disponível..."
    update_info=$(docker compose exec -T "$CONTAINER" wget -q -O - http://127.0.0.1:8090/api/update 2>/dev/null || true)
    if [ -n "$update_info" ]; then
        TARGET_VERSION=$(printf "%s" "$update_info" | grep -o '"latest":"[^"]*"' | cut -d'"' -f4 | sed 's/^v//' || true)
    fi
    if [ -z "$TARGET_VERSION" ]; then
        REPO="${LEARNVAULT_GITHUB_REPO:-AndersonTavares0/LearnVault}"
        latest_release=$(wget -q -O - -U "LearnVault-Updater" "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)
        if [ -n "$latest_release" ]; then
            TARGET_VERSION=$(printf "%s" "$latest_release" | grep -o '"tag_name":"[^"]*"' | cut -d'"' -f4 | sed 's/^v//' || true)
        fi
    fi
    if [ -n "$TARGET_VERSION" ]; then
        log "Nova versão identificada: ${TARGET_VERSION}"
    else
        log "Nenhuma nova versão identificada ou serviço temporariamente indisponível."
        exit 0
    fi
fi

if [ "$CURRENT_VERSION" = "$TARGET_VERSION" ]; then
    log "O LearnVault já está na versão ${TARGET_VERSION}. Nenhuma ação necessária."
    exit 0
fi

log "Preparando atualização de ${CURRENT_VERSION} para ${TARGET_VERSION}..."

# 2. Backup pré-atualização com validação de integridade
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
BACKUP_PATH="/data/backups/pre-update-${CURRENT_VERSION}-${TIMESTAMP}.sqlite"

log "Criando backup pré-atualização em ${BACKUP_PATH}..."
if ! docker compose exec -T "$CONTAINER" /app/learnvault-server backup --database "$DATABASE_PATH" --output "$BACKUP_PATH"; then
    log "ERRO: Falha ao gerar o backup pré-atualização. Atualização abortada para segurança dos dados."
    record_history "failed_pre_backup" "$CURRENT_VERSION" "$TARGET_VERSION" "Falha no backup pré-atualização"
    exit 1
fi
log "Backup preventivo criado com sucesso."

# 3. Baixa imagem da nova versão
NEW_IMAGE="${LEARNVAULT_IMAGE_REPO:-ghcr.io/andersontavares0/learnvault}:${TARGET_VERSION}"
log "Baixando nova imagem: ${NEW_IMAGE}..."

if ! docker pull "$NEW_IMAGE" 2>/dev/null; then
    log "AVISO: Download de ${NEW_IMAGE} falhou ou imagem local. Tentando docker compose pull..."
    docker compose pull learnvault || true
fi

# 4. Aplica a nova imagem
PREVIOUS_IMAGE=$(docker inspect --format='{{.Config.Image}}' "$CONTAINER" 2>/dev/null || echo "")
log "Reiniciando serviços com a nova versão..."
LEARNVAULT_IMAGE="$NEW_IMAGE" docker compose up -d "$CONTAINER"

# 5. Validação de Healthcheck com timeout
log "Aguardando inicialização e validação de healthcheck (até 60s)..."
attempts=0
max_attempts=30
healthy=0

while [ "$attempts" -lt "$max_attempts" ]; do
    status=$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}unknown{{end}}' "$CONTAINER" 2>/dev/null || echo "unhealthy")
    if [ "$status" = "healthy" ]; then
        healthy=1
        break
    fi
    attempts=$((attempts + 1))
    sleep 2
done

if [ "$healthy" -eq 1 ]; then
    new_health=$(docker compose exec -T "$CONTAINER" wget -q -O - http://127.0.0.1:8090/api/healthz 2>/dev/null || true)
    NEW_ACTIVE_VERSION=$(printf "%s" "$new_health" | grep -o '"version":"[^"]*"' | cut -d'"' -f4 || echo "$TARGET_VERSION")
    log "SUCESSO: LearnVault atualizado para a versão ${NEW_ACTIVE_VERSION} e validado com sucesso!"
    record_history "success" "$CURRENT_VERSION" "$NEW_ACTIVE_VERSION" "Atualização concluída com sucesso"
    exit 0
else
    # 6. ROLLBACK AUTOMÁTICO SE FALHAR
    log "ALERTA CRÍTICO: Healthcheck falhou na nova versão! Iniciando reversão segura (rollback)..."
    
    # Para o container com falha
    docker compose stop "$CONTAINER"
    
    # Restaura o banco de dados pré-atualização
    log "Restaurando banco de dados a partir do backup preventivo ${BACKUP_PATH}..."
    docker compose run --rm "$CONTAINER" /app/learnvault-server restore --backup "$BACKUP_PATH" --database "$DATABASE_PATH"
    
    # Reverte para a imagem anterior
    if [ -n "$PREVIOUS_IMAGE" ]; then
        log "Restaurando imagem anterior: ${PREVIOUS_IMAGE}..."
        LEARNVAULT_IMAGE="$PREVIOUS_IMAGE" docker compose up -d "$CONTAINER"
    else
        docker compose start "$CONTAINER"
    fi
    
    log "Reversão concluída. O banco de dados e a versão anterior foram restaurados."
    record_history "rollback" "$CURRENT_VERSION" "$TARGET_VERSION" "Rollback executado após falha de healthcheck na nova versão"
    exit 1
fi
