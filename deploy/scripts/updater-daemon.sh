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

if [ "$AUTO_UPDATE" != "true" ]; then
    log "AUTO_UPDATE está desativado (padrão seguro). O container updater ficará ocioso."
    log "Para habilitar atualizações automáticas, defina AUTO_UPDATE=true no .env."
    # Dorme aguardando sinal
    while true; do sleep 86400; done
fi

# Assegura que o plugin do docker compose está presente se executando em Alpine/docker:cli
if ! docker compose version >/dev/null 2>&1; then
    if command -v apk >/dev/null 2>&1; then
        log "Instalando plugin do Docker Compose no container updater..."
        apk add --no-cache docker-cli-compose >/dev/null 2>&1 || true
    fi
fi

log "Iniciando daemon de atualização automática (Janela: ${AUTO_UPDATE_WINDOW}, Intervalo: ${CHECK_INTERVAL_SECONDS}s)..."

in_window() {
    # Janela no formato HH:MM-HH:MM
    window="$1"
    start_hour=$(printf "%s" "$window" | cut -d'-' -f1 | cut -d':' -f1)
    end_hour=$(printf "%s" "$window" | cut -d'-' -f2 | cut -d':' -f1)
    current_hour=$(date +%H)
    
    # Compara horas como inteiros
    if [ "$current_hour" -ge "$start_hour" ] && [ "$current_hour" -lt "$end_hour" ]; then
        return 0
    fi
    return 1
}

while true; do
    if in_window "$AUTO_UPDATE_WINDOW"; then
        log "Dentro da janela de manutenção (${AUTO_UPDATE_WINDOW}). Verificando atualizações..."
        /scripts/update.sh || log "Aviso: processo de atualização retornou erro ou nenhuma atualização necessária."
    else
        log "Fora da janela de manutenção (${AUTO_UPDATE_WINDOW}). Aguardando próxima verificação..."
    fi
    sleep "$CHECK_INTERVAL_SECONDS"
done
