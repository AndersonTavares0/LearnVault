#!/bin/sh
set -eu

compose=${COMPOSE:-docker compose}
if sh -c "$compose run --rm --no-deps --entrypoint sh learnvault -c 'touch /library/.learnvault-write-probe'"; then
	printf '%s\n' 'ERROR: write to /library unexpectedly succeeded' >&2
	exit 1
fi
printf '%s\n' 'PASS: container cannot write to /library'
