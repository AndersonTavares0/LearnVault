#!/bin/sh
# Takes a safe, transactional backup of LearnVault's SQLite database.
# Works both inside a container and on the host against the running container.
set -eu

CONTAINER="${CONTAINER:-learnvault}"
DATABASE_PATH="${DATABASE_PATH:-/data/learnvault.sqlite}"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
OUTPUT_NAME="backup-${TIMESTAMP}.sqlite"
TARGET_OUTPUT="${1:-/data/backups/${OUTPUT_NAME}}"

printf "Starting the LearnVault backup...\n"

if [ -f "/app/learnvault-server" ]; then
    # Running inside the container
    mkdir -p "$(dirname "$TARGET_OUTPUT")"
    /app/learnvault-server backup --database "$DATABASE_PATH" --output "$TARGET_OUTPUT"
else
    # Running on the host via docker compose
    docker compose exec -T "$CONTAINER" /app/learnvault-server backup --database "$DATABASE_PATH" --output "$TARGET_OUTPUT"
fi

printf "Backup completed successfully: %s\n" "$TARGET_OUTPUT"
