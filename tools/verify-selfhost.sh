#!/bin/sh
# Rebuild the host compiler, then exercise its public CLI and target runtimes in parallel.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
if [ "${1:-}" = --linux ]; then
    case "${2:-}" in
        arm64) platform=linux/arm64 ;;
        x86_64) platform=linux/amd64 ;;
        *) echo 'usage: tools/verify-selfhost.sh --linux arm64|x86_64 [verification flags]' >&2; exit 2 ;;
    esac
    arch=$2
    shift 2
    [ "$(uname -s)" = Darwin ] || { echo 'On Linux, run tools/verify-selfhost.sh directly.' >&2; exit 2; }
    kiri=$(command -v kiri) || kiri=/Applications/Kiri.app/Contents/MacOS/kiri
    [ -x "$kiri" ] || { echo 'Install and start Kiri to run Linux tests on macOS.' >&2; exit 1; }
    image=${DUNDER_KIRI_IMAGE:-dunder-linux-test:$arch}
    if [ -z "${DUNDER_KIRI_IMAGE:-}" ] && ! "$kiri" image inspect "$image" >/dev/null 2>&1; then
        "$kiri" build --platform "$platform" --load -t "$image" - <<'IMAGE'
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    binutils-multiarch build-essential ca-certificates clang curl file git gzip \
    libargon2-dev libcurl4-openssl-dev libsqlite3-dev libssl-dev nodejs openssl \
    && rm -rf /var/lib/apt/lists/*
IMAGE
    fi
    mkdir -p build
    set -- "$image" sh -ec '
        case "$1:$(uname -m)" in arm64:aarch64|arm64:arm64|x86_64:x86_64) ;;
            *) echo "Kiri guest architecture does not match $1" >&2; exit 1 ;;
        esac
        shift
        exec ./tools/verify-selfhost.sh "$@"
    ' verify-linux "$arch" "$@"
    if [ -f .git ]; then
        common=$(git rev-parse --path-format=absolute --git-common-dir)
        set -- --mount "type=bind,source=$common,target=$common,readonly" "$@"
    fi
    exec "$kiri" run --rm --platform "$platform" --workdir /workspace \
        --mount "type=bind,source=$root,target=/workspace,readonly" \
        --mount type=volume,target=/workspace/build,volume-nocopy "$@"
fi

compiler=$(./tools/bootstrap.sh)
work=$(mktemp -d "${TMPDIR:-/tmp}/dunder-verify.XXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM
"$compiler" run selfhost/embed.ddr -o "$work/embed" -- --check &
embed_pid=$!
"$compiler" build tools/verify.ddr -o "$work/verify" >&2 &
verify_pid=$!
embed_rc=0; wait "$embed_pid" || embed_rc=$?
verify_rc=0; wait "$verify_pid" || verify_rc=$?
[ "$embed_rc" -eq 0 ]
[ "$verify_rc" -eq 0 ]
"$work/verify" "$compiler" --mir-reports "$work/mir-reports" "$@"
