#!/bin/sh
# Fast language-development loop: incremental 1-stage compiler build + parallel core suites.
# Usage:
#   ./tools/dev.sh                    # rebuild dev compiler + run fast core suites in parallel
#   ./tools/dev.sh --filter <substr>  # rebuild dev compiler + run all matching suites in parallel
#   ./tools/dev.sh --full             # rebuild dev compiler + run full verification matrix in parallel
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

case "$(uname -s):$(uname -m)" in
    Darwin:arm64) host=arm64-macos; entry=main_parallel ;;
    Linux:aarch64|Linux:arm64) host=arm64-linux; entry=linux ;;
    Linux:x86_64) host=x86_64-linux; entry=x86_linux ;;
    *) echo "Unsupported host: $(uname -sm)" >&2; exit 1 ;;
esac

host_compiler="$root/build/selfhost/$host/dunder"
if [ ! -x "$host_compiler" ]; then
    host_compiler=$(./tools/bootstrap.sh)
fi

# Auto-refresh bundled stdlib when any stdlib source was edited.
regen=0
if [ "stdlib/modules.txt" -nt "selfhost/bundled.ddr" ]; then
    regen=1
else
    for f in stdlib/*.ddr; do
        if [ "$f" -nt "selfhost/bundled.ddr" ]; then
            regen=1
            break
        fi
    done
fi
if [ "$regen" -eq 1 ]; then
    "$host_compiler" run selfhost/embed.ddr -o build/dev/embed
fi

mkdir -p build/dev
dev_compiler="$root/build/dev/dunder"
"$host_compiler" build "selfhost/$entry.ddr" --incremental -o "$dev_compiler" >&2
"$dev_compiler" build tools/verify.ddr --incremental -o build/dev/verify >&2

mode="--fast"
for arg in "$@"; do
    case "$arg" in
        --full|--filter|--fast) mode="" ;;
    esac
done

if [ -n "$mode" ]; then
    exec "$root/build/dev/verify" "$dev_compiler" "$mode" "$@"
else
    exec "$root/build/dev/verify" "$dev_compiler" "$@"
fi
