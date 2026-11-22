#!/bin/sh
# Rebuild the host compiler, then exercise its public CLI and target runtimes in parallel.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
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
