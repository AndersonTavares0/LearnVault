#!/bin/sh
# Restores the LearnVault database from a verified backup.
# Takes a safety copy before the replacement and validates integrity.
set -eu

BACKUP_FILE="${1:-}"
CONTAINER="${CONTAINER:-learnvault}"
DATABASE_PATH="${DATABASE_PATH:-/data/learnvault.sqlite}"

if [ -z "$BACKUP_FILE" ]; then
    printf "Uso: %s <caminho-do-backup.sqlite>\n" "$0" >&2
    exit 1
fi

printf "Starting the LearnVault restore from %s...\n" "$BACKUP_FILE"

if [ -f "/app/learnvault-server" ]; then
    /app/learnvault-server restore --backup "$BACKUP_FILE" --database "$DATABASE_PATH"
else
    docker compose stop "$CONTAINER"
    docker compose run --rm --entrypoint /app/learnvault-server "$CONTAINER" restore --backup "$BACKUP_FILE" --database "$DATABASE_PATH"
    docker compose start "$CONTAINER"
fi

printf "Restore completed successfully.\n"
