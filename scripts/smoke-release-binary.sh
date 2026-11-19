#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <binary-path> <expected-tag> <web-root>" >&2
  exit 2
fi

binary_path="$1"
expected_tag="$2"
web_root="$3"

if [[ ! -x "$binary_path" ]]; then
  echo "::error::Smoke test binary is missing or not executable: $binary_path" >&2
  exit 1
fi
if [[ ! -d "$web_root" ]]; then
  echo "::error::Smoke test web root directory is missing: $web_root" >&2
  exit 1
fi
# Release builds inject LEARNVAULT_BUILD_VERSION="$RELEASE_TAG" verbatim (preserving any leading 'v').
# Accept either vX.Y.Z or bare X.Y.Z as expected_tag, and require the binary to report that exact string.
if [[ ! "$expected_tag" =~ ^v?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "::error::Unexpected expected_tag format: $expected_tag" >&2
  exit 2
fi

smoke_dir="$(mktemp -d)"
smoke_pid=""
cleanup() {
  if [[ -n "$smoke_pid" ]]; then
    kill "$smoke_pid" 2>/dev/null || true
    wait "$smoke_pid" 2>/dev/null || true
  fi
  rm -rf "$smoke_dir"
}
trap cleanup EXIT

smoke_ver="$("$binary_path" --version)"
expected_ver_line="learnvault-server ${expected_tag}"
if [[ "$smoke_ver" != "$expected_ver_line" ]]; then
  echo "::error::Unexpected binary version (expected '${expected_ver_line}', got '${smoke_ver}')" >&2
  exit 1
fi

mkdir -p "$smoke_dir/lib"
ADMIN_USERNAME=admin ADMIN_PASSWORD=smoke-test-password-1234 \
  "$binary_path" \
    --listen-addr 127.0.0.1:0 \
    --database "$smoke_dir/db.sqlite" \
    --library-root "$smoke_dir/lib" \
    --web-root "$web_root" \
    --insecure-http \
    --no-update-check \
    >"$smoke_dir/stdout.log" 2>"$smoke_dir/stderr.log" &
smoke_pid=$!

ready_line=""
for _ in $(seq 1 100); do
  if grep -m1 '^LEARNVAULT_READY ' "$smoke_dir/stdout.log" >"$smoke_dir/ready"; then
    ready_line="$(cat "$smoke_dir/ready")"
    break
  fi
  if ! kill -0 "$smoke_pid" 2>/dev/null; then
    echo "::error::${binary_path} exited before becoming ready" >&2
    cat "$smoke_dir/stderr.log" >&2 || true
    exit 1
  fi
  sleep 0.1
done

if [[ -z "$ready_line" ]]; then
  echo "::error::${binary_path} did not emit LEARNVAULT_READY within 10s" >&2
  cat "$smoke_dir/stderr.log" >&2 || true
  exit 1
fi

addr="${ready_line#LEARNVAULT_READY }"
health_json="$(curl -fsS "http://${addr}/api/healthz")"
if ! printf '%s' "$health_json" | jq -e --arg v "$expected_tag" '.status == "ok" and .version == $v' >/dev/null; then
  echo "::error::Unexpected /api/healthz response (expected status=ok, version=${expected_tag}): ${health_json}" >&2
  exit 1
fi
