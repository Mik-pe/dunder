#!/bin/sh
# Keep task syntax outside the frozen seed's serial dependency graph. Build
# the shared serial compiler first, then require serial/parallel producers to
# emit exactly the same task-capable compiler image before publishing it.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) ;;
    *) echo 'Parallel compiler bootstrap requires an ARM64/macOS host' >&2; exit 1 ;;
esac
if [ "$#" -eq 0 ]; then
    serial_compiler=$(./tools/bootstrap.sh)
elif [ "$#" -eq 2 ] && [ "$1" = --compiler ]; then
    serial_compiler=$2
else
    echo 'usage: tools/bootstrap-parallel.sh [--compiler SERIAL_COMPILER]' >&2
    exit 2
fi
parallel_dir=build/selfhost/arm64-macos
mkdir -p "$parallel_dir"
parallel_work=$(mktemp -d "$parallel_dir/.parallel.XXXXXX")
parallel_success=0
trap 'if [ "$parallel_success" = 1 ]; then rm -rf "$parallel_work"; else echo "Retained failed parallel bootstrap: $parallel_work" >&2; fi' EXIT
trap 'exit 1' HUP INT TERM
"$serial_compiler" build selfhost/main_parallel.ddr -o "$parallel_work/serial-produced" >&2
"$parallel_work/serial-produced" build selfhost/main_parallel.ddr --jobs 1 -o "$parallel_work/one-worker" >&2
cmp "$parallel_work/serial-produced" "$parallel_work/one-worker"
"$parallel_work/one-worker" build selfhost/main_parallel.ddr --jobs 4 -o "$parallel_work/four-workers" >&2
cmp "$parallel_work/one-worker" "$parallel_work/four-workers"
mv -f "$parallel_work/four-workers" "$parallel_dir/dunder-parallel"
parallel_success=1
printf '%s\n' "$root/$parallel_dir/dunder-parallel"
