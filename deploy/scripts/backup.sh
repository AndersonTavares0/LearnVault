#!/bin/sh
# Realiza backup seguro e transacional do banco de dados SQLite do LearnVault.
# Funciona tanto dentro de um container quanto no host contra o container ativo.
set -eu

CONTAINER="${CONTAINER:-learnvault}"
DATABASE_PATH="${DATABASE_PATH:-/data/learnvault.sqlite}"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
OUTPUT_NAME="backup-${TIMESTAMP}.sqlite"
TARGET_OUTPUT="${1:-/data/backups/${OUTPUT_NAME}}"

printf "Iniciando backup seguro do LearnVault...\n"

if [ -f "/app/learnvault-server" ]; then
    # Executando internamente no container
    mkdir -p "$(dirname "$TARGET_OUTPUT")"
    /app/learnvault-server backup --database "$DATABASE_PATH" --output "$TARGET_OUTPUT"
else
    # Executando no host via docker compose
    docker compose exec -T "$CONTAINER" /app/learnvault-server backup --database "$DATABASE_PATH" --output "$TARGET_OUTPUT"
fi

printf "Backup concluído com sucesso: %s\n" "$TARGET_OUTPUT"
