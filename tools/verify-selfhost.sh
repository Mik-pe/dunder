#!/bin/sh
# Rebuild the host compiler, then exercise its public CLI and target runtimes in parallel.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
compiler=$(./tools/bootstrap.sh)
work=$(mktemp -d "${TMPDIR:-/tmp}/dunder-verify.XXXXXX")
# Keep each metadata matrix's evidence independently. Its tools require
# empty outputs so rerunning verification must never reuse an earlier run.
mir_reports=$(mktemp -d "$root/build/mir-correctness.XXXXXX")
printf 'MIR verification evidence: %s\n' "$mir_reports"
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM
serial_compiler="$root/build/selfhost/arm64-macos/dunder-serial"
"$compiler" build tools/verify.ddr -o "$work/verify" >&2
"$work/verify" "$compiler" --mir-reports "$mir_reports" --serial-compiler "$serial_compiler" "$@"
