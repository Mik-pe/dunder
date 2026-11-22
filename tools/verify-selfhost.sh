#!/bin/sh
# Rebuild the host compiler, then exercise its public CLI and target runtimes in parallel.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
compiler=$(./tools/bootstrap.sh)
work=$(mktemp -d "${TMPDIR:-/tmp}/dunder-verify.XXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) host=arm64-macos ;;
    Linux:aarch64|Linux:arm64) host=arm64-linux ;;
    Linux:x86_64) host=x86_64-linux ;;
    *) echo "Unsupported verify host: $(uname -sm)" >&2; exit 1 ;;
esac
serial_compiler="$root/build/selfhost/$host/dunder-serial"
"$compiler" run selfhost/embed.ddr -o "$work/embed" -- --check &
embed_pid=$!
"$compiler" build tools/verify.ddr -o "$work/verify" >&2 &
verify_pid=$!
embed_rc=0; wait "$embed_pid" || embed_rc=$?
verify_rc=0; wait "$verify_pid" || verify_rc=$?
[ "$embed_rc" -eq 0 ] && [ "$verify_rc" -eq 0 ]
"$work/verify" "$compiler" --mir-reports "$work/mir-reports" --serial-compiler "$serial_compiler" "$@"
