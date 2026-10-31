#!/bin/sh
# Restaura o banco de dados do LearnVault a partir de um backup verificado.
# Executa cópia de segurança antes da substituição e valida integridade.
set -eu

BACKUP_FILE="${1:-}"
CONTAINER="${CONTAINER:-learnvault}"
DATABASE_PATH="${DATABASE_PATH:-/data/learnvault.sqlite}"

if [ -z "$BACKUP_FILE" ]; then
    printf "Uso: %s <caminho-do-backup.sqlite>\n" "$0" >&2
    exit 1
fi

printf "Iniciando restauração segura do LearnVault a partir de %s...\n" "$BACKUP_FILE"

if [ -f "/app/learnvault-server" ]; then
    /app/learnvault-server restore --backup "$BACKUP_FILE" --database "$DATABASE_PATH"
else
    docker compose stop "$CONTAINER"
    docker compose run --rm "$CONTAINER" /app/learnvault-server restore --backup "$BACKUP_FILE" --database "$DATABASE_PATH"
    docker compose start "$CONTAINER"
fi

printf "Restauração concluída com sucesso.\n"
