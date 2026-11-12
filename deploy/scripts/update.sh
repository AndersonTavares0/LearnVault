#!/bin/sh
# LearnVault - standalone update script with backup and safe rollback
#
# Steps:
# 1. Find the current version and the target version
# 2. Check that the target image is signed by this project's release workflow (cosign)
# 3. Take a pre-update backup and check its integrity
# 4. Pull the new, versioned image (by the verified digest)
# 5. Update the containers with Docker Compose
# 6. Check the healthcheck and the migrations after start
# 7. On failure: roll back to the previous image and database automatically
# 8. Record a structured history in /data/update-history.json
set -eu

CONTAINER="${LEARNVAULT_CONTAINER:-learnvault}"
DATABASE_PATH="/data/learnvault.sqlite"
DATA_DIR="${DATA_DIR:-/data}"
HISTORY_FILE="${DATA_DIR}/update-history.json"
TARGET_VERSION="${1:-}"

# Serialize updates sharing the same persistent data directory.
mkdir -p "$DATA_DIR"
UPDATE_LOCK="${DATA_DIR}/.update.lock"
if ! mkdir "$UPDATE_LOCK" 2>/dev/null; then
    printf '%s\n' "Another update holds $UPDATE_LOCK; update aborted." >&2
    exit 1
fi
# shortcut: a crash can leave this lock behind; add supervisor-owned locking for automatic crash recovery.
trap 'rmdir "$UPDATE_LOCK"' 0

# Prints the certificate identity a valid release signature must have: this repository's
# release workflow, run for a version tag or for main. $1 is owner/name, already validated.
signer_identity_regex() {
    owner_name=$(printf '%s' "$1" | sed 's/[.]/\\./g')
    printf '%s' "^https://github\\.com/${owner_name}/\\.github/workflows/release\\.yml@refs/(tags/v[0-9]+\\.[0-9]+\\.[0-9]+|heads/main)\$"
}

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

TARGET_VERSION=${TARGET_VERSION#v}
CURRENT_VERSION=${CURRENT_VERSION#v}
if ! printf '%s\n' "$TARGET_VERSION" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; then
    log "Invalid stable target version; update aborted."
    exit 1
fi

if [ "$CURRENT_VERSION" = "$TARGET_VERSION" ]; then
    log "O LearnVault já está na versão ${TARGET_VERSION}. Nenhuma ação necessária."
    exit 0
fi

log "Preparando atualização de ${CURRENT_VERSION} para ${TARGET_VERSION}..."

# 1b. Only an image signed by this project's release workflow is accepted (cosign keyless).
# The signature is checked before anything is backed up, downloaded or changed, and the image
# that is then pulled and started is the verified digest, not the mutable tag. Set
# LEARNVAULT_VERIFY_SIGNATURE=false only for a private mirror that re-publishes the image.
IMAGE_REPO="${LEARNVAULT_IMAGE_REPO:-ghcr.io/andersontavares0/learnvault}"
SIGNER_REPO="${LEARNVAULT_GITHUB_REPO:-AndersonTavares0/LearnVault}"
VERIFY_SIGNATURE="${LEARNVAULT_VERIFY_SIGNATURE:-true}"
VERIFIED_IMAGE=""
case "$VERIFY_SIGNATURE" in
    true|false) ;;
    *)
        log "LEARNVAULT_VERIFY_SIGNATURE must be true or false; update aborted."
        exit 1 ;;
esac
if [ "$VERIFY_SIGNATURE" = "true" ]; then
    if ! printf '%s\n' "$SIGNER_REPO" | grep -Eq '^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$'; then
        log "LEARNVAULT_GITHUB_REPO must look like owner/name; update aborted."
        exit 1
    fi
    if ! command -v cosign >/dev/null 2>&1; then
        # shortcut: cosign is installed from the Alpine mirror at run time, unpinned; bake a pinned cosign into the updater image.
        apk add --no-cache cosign >/dev/null 2>&1 || true
    fi
    if ! command -v cosign >/dev/null 2>&1; then
        log "cosign is not available, so the image signature cannot be checked; update aborted."
        record_history "failed_verification" "$CURRENT_VERSION" "$TARGET_VERSION" "cosign unavailable; signature not checked"
        exit 1
    fi
    verify_errors="${DATA_DIR}/.cosign-verify.err"
    if ! verified=$(cosign verify "${IMAGE_REPO}:${TARGET_VERSION}" \
        --certificate-oidc-issuer https://token.actions.githubusercontent.com \
        --certificate-identity-regexp "$(signer_identity_regex "$SIGNER_REPO")" 2>"$verify_errors"); then
        log "The image ${IMAGE_REPO}:${TARGET_VERSION} has no valid signature from the release workflow; update aborted and the running service is unchanged."
        sed -n '1,3p' "$verify_errors" 2>/dev/null | while IFS= read -r line; do log "cosign: $line"; done
        record_history "failed_verification" "$CURRENT_VERSION" "$TARGET_VERSION" "Image signature missing or invalid"
        exit 1
    fi
    verified_digest=$(printf '%s' "$verified" | grep -o '"docker-manifest-digest":"sha256:[0-9a-f]\{64\}"' | head -n 1 | cut -d'"' -f4)
    if [ -z "$verified_digest" ]; then
        log "cosign did not report the verified digest; update aborted and the running service is unchanged."
        record_history "failed_verification" "$CURRENT_VERSION" "$TARGET_VERSION" "Verified digest missing"
        exit 1
    fi
    VERIFIED_IMAGE="${IMAGE_REPO}@${verified_digest}"
    log "Image signature verified (${verified_digest})."
fi

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
NEW_IMAGE="${IMAGE_REPO}:${TARGET_VERSION}"
if [ -n "$VERIFIED_IMAGE" ]; then
    NEW_IMAGE="$VERIFIED_IMAGE"
fi
log "Baixando nova imagem: ${NEW_IMAGE}..."

if ! docker pull "$NEW_IMAGE" 2>/dev/null; then
    log "Target image download failed; the running service is unchanged."
    record_history "failed_download" "$CURRENT_VERSION" "$TARGET_VERSION" "Target image download failed"
    exit 1
fi

# 4. Aplica a nova imagem
# Retain the immutable previous image, not a mutable image tag.
PREVIOUS_IMAGE=$(docker inspect --format='{{.Image}}' "$CONTAINER" 2>/dev/null || true)
if [ -z "$PREVIOUS_IMAGE" ]; then
    log "Could not identify the previous image; update aborted."
    exit 1
fi
log "Starting target image..."
started=0
if LEARNVAULT_IMAGE="$NEW_IMAGE" docker compose up -d --no-build "$CONTAINER"; then
    started=1
fi

# 5. Validação de Healthcheck com timeout
log "Aguardando inicialização e validação de healthcheck (até 60s)..."
attempts=0
max_attempts=30
healthy=0

while [ "$started" -eq 1 ] && [ "$attempts" -lt "$max_attempts" ]; do
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
    NEW_ACTIVE_VERSION=$(printf "%s" "$new_health" | grep -o '"version":"[^"]*"' | cut -d'"' -f4 || true)
    NEW_ACTIVE_VERSION=${NEW_ACTIVE_VERSION#v}
    [ "$NEW_ACTIVE_VERSION" = "$TARGET_VERSION" ] || healthy=0
fi

if [ "$healthy" -eq 1 ]; then
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
    LEARNVAULT_IMAGE="$PREVIOUS_IMAGE" docker compose run --rm --entrypoint /app/learnvault-server "$CONTAINER" restore --backup "$BACKUP_PATH" --database "$DATABASE_PATH"
    
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
