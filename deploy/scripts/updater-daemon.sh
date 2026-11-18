#!/bin/sh
# LearnVault - opt-in automatic update daemon
# Respects the configured maintenance window (e.g. AUTO_UPDATE_WINDOW="03:00-05:00")
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
    log "AUTO_UPDATE is off (the safe default). The updater container will stay idle."
    log "To enable automatic updates, set AUTO_UPDATE=true in .env."
    # Sleeps, waiting for a signal
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
# Ensures the docker compose plugin is present when running on Alpine/docker:cli
if ! docker compose version >/dev/null 2>&1; then
    if command -v apk >/dev/null 2>&1; then
        log "Installing the Docker Compose plugin in the updater container..."
        apk add --no-cache docker-cli-compose >/dev/null 2>&1 || true
    fi
fi

log "Starting the automatic update daemon (window: ${AUTO_UPDATE_WINDOW}, interval: ${CHECK_INTERVAL_SECONDS}s)..."


while true; do
    if in_window "$AUTO_UPDATE_WINDOW"; then
        log "Inside the maintenance window (${AUTO_UPDATE_WINDOW}). Checking for updates..."
        /scripts/update.sh || log "Warning: the update process returned an error or no update was needed."
    else
        log "Outside the maintenance window (${AUTO_UPDATE_WINDOW}). Waiting for the next check..."
    fi
    sleep "$CHECK_INTERVAL_SECONDS"
done
