#!/bin/sh
# Keep task syntax outside the frozen seed's serial dependency graph. Build
# the shared serial compiler first, then require serial/parallel producers to
# emit exactly the same task-capable compiler image before publishing it.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) host=arm64-macos ;;
    Linux:aarch64|Linux:arm64) host=arm64-linux ;;
    Linux:x86_64) host=x86_64-linux ;;
    *) echo "Unsupported parallel bootstrap host: $(uname -sm)" >&2; exit 1 ;;
esac
if [ "$#" -eq 0 ]; then
    serial_compiler=$(./tools/bootstrap.sh --serial)
elif [ "$#" -eq 2 ] && [ "$1" = --compiler ]; then
    serial_compiler=$2
else
    echo 'usage: tools/bootstrap-parallel.sh [--compiler SERIAL_COMPILER]' >&2
    exit 2
fi
parallel_dir="build/selfhost/$host"
mkdir -p "$parallel_dir"
parallel_work=$(mktemp -d "$parallel_dir/.parallel.XXXXXX")
parallel_success=0
trap 'if [ "$parallel_success" = 1 ]; then rm -rf "$parallel_work"; else echo "Retained failed parallel bootstrap: $parallel_work" >&2; fi' EXIT
trap 'exit 1' HUP INT TERM
"$serial_compiler" build selfhost/main_parallel.ddr -o "$parallel_work/serial-produced" >&2
"$parallel_work/serial-produced" build selfhost/main_parallel.ddr --jobs 4 -o "$parallel_work/four-workers" >&2
cmp "$parallel_work/serial-produced" "$parallel_work/four-workers"
mv -f "$parallel_work/four-workers" "$parallel_dir/dunder-parallel"
parallel_success=1
printf '%s\n' "$root/$parallel_dir/dunder-parallel"
