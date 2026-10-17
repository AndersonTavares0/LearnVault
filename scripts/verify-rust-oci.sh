#!/bin/sh
#
# Experimental OCI validation for the opt-in Rust server (`crates/server`).
#
# This is a manual, non-publishing validation. It builds
# `deploy/container/Containerfile.rust`, runs the image against a synthetic library
# fixture and checks that:
#   - the packaged binary runs and reports its version;
#   - `GET /api/healthz` answers 200 with the expected payload and `no-store`;
#   - the image HEALTHCHECK reaches `healthy`;
#   - API routes without a session stay unauthenticated (401);
#   - the server runs as the non-root uid 10001, with `no-new-privileges` set and
#     all capabilities dropped;
#   - the data directory is writable (the SQLite database file is created);
#   - the mounted library is readable, mounted read-only and not writable;
#   - the container root filesystem rejects writes;
#   - the server recovers after a restart and the data persists;
#   - the validation container is removed on cleanup.
#
# The Rust server currently exposes only a database path and a library root; it has
# no cache-directory interface yet, so there is no cache mount to validate. The script
# records that limitation instead of assuming a `CACHE_DIR` that the binary ignores.
#
# It never reads `/mnt/cursos` or any real library: the fixture is synthetic, generated
# in a temporary directory and removed on exit. Nothing is published or pushed.
#
# Requirements:
#   - Docker or Podman on PATH (override with `CONTAINER_ENGINE`, e.g. `podman`);
#   - network access for the Rust dependency download during the image build.
#
# Usage:
#   ./scripts/verify-rust-oci.sh
#   CONTAINER_ENGINE=podman ./scripts/verify-rust-oci.sh
#   KEEP=1 ./scripts/verify-rust-oci.sh   # keep the temp fixture for debugging
#
set -eu

# Git Bash/MSYS rewrites absolute POSIX arguments (for example `/library` or
# `/data/learnvault.sqlite`) into Windows paths before `docker.exe` receives them,
# which corrupts container-side paths. Disabling that keeps container paths literal.
# The variable is ignored on Linux/macOS.
MSYS_NO_PATHCONV=1
export MSYS_NO_PATHCONV

IMAGE="${IMAGE:-learnvault-rust-validation:local}"
CONTAINER="${CONTAINER:-learnvault-rust-validation}"
PORT="${PORT:-8090}"
ADMIN_USERNAME="${ADMIN_USERNAME:-admin}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-learnvault-validation}"
KEEP="${KEEP:-0}"

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

# Translate POSIX paths to the host syntax the container engine expects when running
# under an MSYS/Cygwin shell (for example Git Bash on Windows).
host_path() {
	if command -v cygpath >/dev/null 2>&1; then
		cygpath -w -- "$1"
	else
		printf '%s' "$1"
	fi
}

engine="${CONTAINER_ENGINE:-}"
if [ -z "$engine" ]; then
	if command -v docker >/dev/null 2>&1; then
		engine=docker
	elif command -v podman >/dev/null 2>&1; then
		engine=podman
	else
		printf '%s\n' 'ERROR: this validation requires Docker or Podman.' >&2
		printf '%s\n' 'Install one, or set CONTAINER_ENGINE to the engine binary.' >&2
		exit 2
	fi
elif ! command -v "$engine" >/dev/null 2>&1; then
	printf 'ERROR: CONTAINER_ENGINE=%s was not found on PATH.\n' "$engine" >&2
	exit 2
fi

printf 'Using container engine: %s\n' "$engine"

workdir=$(mktemp -d)
library="$workdir/library"
data="$workdir/data"
started=0

cleanup() {
	status=$?
	trap - EXIT INT TERM
	if [ "$started" = 1 ]; then
		"$engine" rm -f "$CONTAINER" >/dev/null 2>&1 || true
	fi
	if [ "$KEEP" = 1 ]; then
		printf 'Kept temporary fixture at %s (KEEP=1)\n' "$workdir"
	else
		rm -rf "$workdir"
	fi
	exit "$status"
}
trap cleanup EXIT INT TERM

# Synthetic fixture: tiny text payloads, never real course media.
mkdir -p "$library/course" "$data"
printf '%s\n' 'synthetic validation fixture' > "$library/course/lesson.mp4"
printf '%s\n' 'synthetic subtitle' > "$library/course/lesson.srt"
chmod -R a+rX "$library" 2>/dev/null || true
# The container runs as uid 10001: it must be able to write only to /data.
chmod 0777 "$data" 2>/dev/null || true
chmod 0755 "$workdir" 2>/dev/null || true

printf 'Building %s ...\n' "$IMAGE"
"$engine" build \
	-f "$(host_path "$repo_root/deploy/container/Containerfile.rust")" \
	-t "$IMAGE" \
	"$(host_path "$repo_root")"

failures=0
pass() { printf 'PASS: %s\n' "$1"; }
fail() {
	printf 'FAIL: %s\n' "$1" >&2
	failures=$((failures + 1))
}
skip() { printf 'SKIP: %s\n' "$1"; }

printf '\nChecking the packaged binary ...\n'
version_output=$("$engine" run --rm "$IMAGE" --version 2>/dev/null || true)
if printf '%s' "$version_output" | grep -q '^learnvault-server '; then
	pass "binary reports its version ($version_output)"
else
	fail "binary did not report a version (got: ${version_output:-<empty>})"
fi

printf '\nStarting the container ...\n'
"$engine" run -d --name "$CONTAINER" \
	--read-only \
	--tmpfs "/tmp:rw,noexec,nosuid,size=16m" \
	--security-opt no-new-privileges \
	--cap-drop ALL \
	-e "ADMIN_USERNAME=$ADMIN_USERNAME" \
	-e "ADMIN_PASSWORD=$ADMIN_PASSWORD" \
	-e "PORT=$PORT" \
	-v "$(host_path "$library"):/library:ro" \
	-v "$(host_path "$data"):/data:rw" \
	"$IMAGE" \
	--listen-addr "0.0.0.0:$PORT" \
	--database /data/learnvault.sqlite \
	--library-root /library \
	>/dev/null
started=1

printf 'Waiting for /api/healthz (up to 30s) ...\n'
health_ok=0
attempt=0
while [ "$attempt" -lt 30 ]; do
	if "$engine" exec "$CONTAINER" \
		wget -q -O - "http://127.0.0.1:$PORT/api/healthz" >"$workdir/health.json" 2>/dev/null; then
		health_ok=1
		break
	fi
	attempt=$((attempt + 1))
	sleep 1
done

printf '\nChecks:\n'
if [ "$health_ok" != 1 ]; then
	fail 'server did not answer /api/healthz within 30s'
	"$engine" logs "$CONTAINER" 2>&1 | tail -n 20 || true
else
	if grep -q '"status":"ok"' "$workdir/health.json"; then
		pass 'GET /api/healthz returns {"status":"ok"}'
	else
		fail "unexpected /api/healthz payload: $(cat "$workdir/health.json")"
	fi
	if "$engine" exec "$CONTAINER" wget -q -S -O /dev/null \
		"http://127.0.0.1:$PORT/api/healthz" 2>&1 | grep -qi 'no-store'; then
		pass '/api/healthz is served with no-store'
	else
		fail '/api/healthz is missing the no-store cache policy'
	fi
	health_status=none
	health_wait=0
	while [ "$health_wait" -lt 60 ]; do
		health_status=$("$engine" inspect --format '{{.State.Health.Status}}' "$CONTAINER" 2>/dev/null || printf 'none')
		if [ "$health_status" = healthy ]; then
			break
		fi
		health_wait=$((health_wait + 1))
		sleep 1
	done
	if [ "$health_status" = healthy ]; then
		pass 'image HEALTHCHECK reaches healthy'
	else
		fail "image HEALTHCHECK did not reach healthy (got: $health_status)"
	fi
	if "$engine" exec "$CONTAINER" wget -q -S -O /dev/null \
		"http://127.0.0.1:$PORT/api/catalog" 2>&1 | grep -q ' 401'; then
		pass '/api/catalog rejects unauthenticated requests (401)'
	else
		fail '/api/catalog did not reject an unauthenticated request with 401'
	fi
	if "$engine" exec "$CONTAINER" wget -q -S -O /dev/null \
		"http://127.0.0.1:$PORT/api/media?path=course/lesson.mp4" 2>&1 | grep -q ' 401'; then
		pass '/api/media rejects unauthenticated requests (401)'
	else
		fail '/api/media did not reject an unauthenticated request with 401'
	fi
fi

if "$engine" exec "$CONTAINER" test -f /data/learnvault.sqlite; then
	pass 'data directory is writable (SQLite database was created)'
else
	fail 'SQLite database was not created in the data directory'
fi

container_uid=$("$engine" exec "$CONTAINER" id -u 2>/dev/null || true)
if [ "$container_uid" = 10001 ]; then
	pass 'server runs as the non-root user (uid 10001)'
else
	fail "server is not running as uid 10001 (got: ${container_uid:-<unknown>})"
fi

if "$engine" exec "$CONTAINER" sh -c 'grep -Eq "^NoNewPrivs:[[:space:]]*1$" /proc/self/status && grep -Eq "^(CapEff|CapBnd):[[:space:]]*0+$" /proc/self/status' >/dev/null 2>&1; then
	pass 'no-new-privileges is set and all capabilities are dropped'
else
	fail 'no-new-privileges or capability dropping is not in effect'
fi

if "$engine" exec "$CONTAINER" test -r /library/course/lesson.mp4; then
	pass 'mounted library content is readable'
else
	fail 'mounted library content is not readable'
fi

if "$engine" exec "$CONTAINER" cat /proc/mounts 2>/dev/null \
	| grep -Eq ' /library .*[[:space:]]ro(,|$)'; then
	pass 'library is mounted read-only'
else
	fail 'library is not mounted read-only'
fi

if "$engine" exec "$CONTAINER" sh -c 'touch /library/.write-probe' >/dev/null 2>&1; then
	fail 'writing to /library succeeded unexpectedly'
else
	pass 'container cannot write to the read-only library'
fi

if "$engine" exec "$CONTAINER" sh -c 'touch /app/.write-probe' >/dev/null 2>&1; then
	fail 'writing to the container root filesystem succeeded unexpectedly'
else
	pass 'container root filesystem is read-only'
fi

skip 'cache directory: the Rust server has no CACHE_DIR interface yet; nothing to mount'

printf '\nRestart check:\n'
db_before=$("$engine" exec "$CONTAINER" stat -c '%d:%i' /data/learnvault.sqlite 2>/dev/null || true)
"$engine" exec "$CONTAINER" sh -c 'printf marker > /data/restart-probe' >/dev/null 2>&1 || true
"$engine" restart "$CONTAINER" >/dev/null
restart_ok=0
restart_wait=0
while [ "$restart_wait" -lt 30 ]; do
	if "$engine" exec "$CONTAINER" wget -q -O - \
		"http://127.0.0.1:$PORT/api/healthz" >/dev/null 2>&1; then
		restart_ok=1
		break
	fi
	restart_wait=$((restart_wait + 1))
	sleep 1
done
if [ "$restart_ok" = 1 ]; then
	pass 'server serves /api/healthz again after a restart'
else
	fail 'server did not recover after a restart'
	"$engine" logs "$CONTAINER" 2>&1 | tail -n 20 || true
fi
db_after=$("$engine" exec "$CONTAINER" stat -c '%d:%i' /data/learnvault.sqlite 2>/dev/null || true)
if [ -n "$db_before" ] && [ "$db_before" = "$db_after" ]; then
	pass 'database file survives a restart (same inode)'
else
	fail "database identity changed across a restart ($db_before -> $db_after)"
fi
if "$engine" exec "$CONTAINER" test -f /data/restart-probe; then
	pass 'data directory survives a restart'
else
	fail 'data directory did not survive a restart'
fi

# Cleanup is also enforced by the EXIT trap; removing it here lets the script
# assert that no container is left behind.
"$engine" rm -f "$CONTAINER" >/dev/null 2>&1 || true
started=0
if "$engine" ps -a --format '{{.Names}}' 2>/dev/null | grep -qx "$CONTAINER"; then
	fail 'validation container was not removed'
else
	pass 'validation container was removed by cleanup'
fi

printf '\nSummary:\n'
if [ "$failures" -eq 0 ]; then
	printf 'PASS: experimental Rust OCI validation completed.\n'
else
	printf 'FAIL: %s check(s) failed.\n' "$failures" >&2
	exit 1
fi
