#!/bin/sh
# LearnVault - Daemon de Atualização Automática Opt-in
# Respeita a janela de manutenção configurada (ex: AUTO_UPDATE_WINDOW="03:00-05:00")
set -eu

AUTO_UPDATE="${AUTO_UPDATE:-false}"
AUTO_UPDATE_WINDOW="${AUTO_UPDATE_WINDOW:-03:00-05:00}"
CHECK_INTERVAL_SECONDS="${CHECK_INTERVAL_SECONDS:-3600}"

log() {
    printf "[%s] [Updater Daemon] %s\n" "$(date +'%Y-%m-%d %H:%M:%S')" "$*"
}

# Return minutes since midnight only for an exact 24-hour clock value.
time_minutes() {
    case "$1" in
        [01][0-9]:[0-5][0-9]|2[0-3]:[0-5][0-9]) ;;
        *) return 2 ;;
    esac
    hour=${1%:*}; minute=${1#*:}
    hour=${hour#0}; minute=${minute#0}
    printf '%s\n' "$(( ${hour:-0} * 60 + ${minute:-0} ))"
}

in_window() {
    window="$1"
    start=$(time_minutes "${window%-*}") || return 2
    end=$(time_minutes "${window#*-}") || return 2
    current=$(time_minutes "${2:-$(date +%H:%M)}") || return 2
    [ "$start" -ne "$end" ] || return 2
    if [ "$start" -lt "$end" ]; then
        [ "$current" -ge "$start" ] && [ "$current" -lt "$end" ]
    else
        [ "$current" -ge "$start" ] || [ "$current" -lt "$end" ]
    fi
}

if [ "$AUTO_UPDATE" != "true" ]; then
    log "AUTO_UPDATE está desativado (padrão seguro). O container updater ficará ocioso."
    log "Para habilitar atualizações automáticas, defina AUTO_UPDATE=true no .env."
    # Dorme aguardando sinal
    while true; do sleep 86400; done
fi

# Reject invalid opt-in settings before any Docker or package-manager operation.
window_status=0
in_window "$AUTO_UPDATE_WINDOW" "00:00" || window_status=$?
if [ "$window_status" -eq 2 ]; then
    log "Invalid AUTO_UPDATE_WINDOW: use distinct HH:MM-HH:MM endpoints."
    exit 1
fi
case "$CHECK_INTERVAL_SECONDS" in
    ''|*[!0-9]*) log "CHECK_INTERVAL_SECONDS must be a positive integer."; exit 1 ;;
esac
if [ "${#CHECK_INTERVAL_SECONDS}" -gt 10 ]; then
    log "CHECK_INTERVAL_SECONDS exceeds the portable integer range."
    exit 1
fi
while [ "${CHECK_INTERVAL_SECONDS#0}" != "$CHECK_INTERVAL_SECONDS" ]; do
    CHECK_INTERVAL_SECONDS=${CHECK_INTERVAL_SECONDS#0}
done
if [ -z "$CHECK_INTERVAL_SECONDS" ] || [ "$CHECK_INTERVAL_SECONDS" -gt 2147483647 ]; then
    log "CHECK_INTERVAL_SECONDS must be between 1 and 2147483647."
    exit 1
fi
# Assegura que o plugin do docker compose está presente se executando em Alpine/docker:cli
if ! docker compose version >/dev/null 2>&1; then
    if command -v apk >/dev/null 2>&1; then
        log "Instalando plugin do Docker Compose no container updater..."
        apk add --no-cache docker-cli-compose >/dev/null 2>&1 || true
    fi
fi

log "Iniciando daemon de atualização automática (Janela: ${AUTO_UPDATE_WINDOW}, Intervalo: ${CHECK_INTERVAL_SECONDS}s)..."


while true; do
    if in_window "$AUTO_UPDATE_WINDOW"; then
        log "Dentro da janela de manutenção (${AUTO_UPDATE_WINDOW}). Verificando atualizações..."
        /scripts/update.sh || log "Aviso: processo de atualização retornou erro ou nenhuma atualização necessária."
    else
        log "Fora da janela de manutenção (${AUTO_UPDATE_WINDOW}). Aguardando próxima verificação..."
    fi
    sleep "$CHECK_INTERVAL_SECONDS"
done
