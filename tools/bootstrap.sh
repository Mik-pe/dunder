#!/bin/sh
# Rebuild Dunder from the frozen compiler for this host.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
serial_only=0
if [ "$#" -eq 1 ] && [ "$1" = --serial ]; then
    serial_only=1
elif [ "$#" -ne 0 ]; then
    echo 'usage: tools/bootstrap.sh [--serial]' >&2
    exit 2
fi
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) host=arm64-macos; entry=main ;;
    Linux:aarch64|Linux:arm64) host=arm64-linux; entry=linux ;;
    Linux:x86_64) host=x86_64-linux; entry=x86_linux ;;
    *) echo "Unsupported bootstrap host: $(uname -sm)" >&2; exit 1 ;;
esac

dir="build/selfhost/$host"
mkdir -p "$dir"
work=$(mktemp -d "$dir/.rebuild.XXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM
(
    cd bootstrap
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c "$host.sha256"
    else
        shasum -a 256 -c "$host.sha256"
    fi
) >&2
gzip -dc "bootstrap/$host.gz" > "$work/seed"
chmod 755 "$work/seed"
"$work/seed" build "selfhost/$entry.ddr" -o "$work/stage2" >&2
if [ "$serial_only" -eq 1 ]; then
    "$work/stage2" build "selfhost/$entry.ddr" -o "$work/stage3" >&2
    if ! cmp -s "$work/stage2" "$work/stage3"; then
        # A new compiler can intentionally change its own emitted code. Rebuild
        # once more with the new emitter, then require an identical fixed point.
        "$work/stage3" build "selfhost/$entry.ddr" -o "$work/stage4" >&2
        cmp "$work/stage3" "$work/stage4"
    fi
    # The frozen seed only needs the serial entry's syntax and dependencies.
    # Keep that verified compiler as a recovery path and producer for the
    # task-capable entry. Never replace the standard binary on a failed build.
    mv -f "$work/stage3" "$dir/dunder-serial"
    printf '%s\n' "$root/$dir/dunder-serial"
    exit 0
fi

# Build the serial entry and task-capable parallel entry concurrently from stage2.
"$work/stage2" build "selfhost/$entry.ddr" -o "$work/stage3" >&2 &
s3_pid=$!
"$work/stage2" build selfhost/main_parallel.ddr -o "$work/stage2-par" >&2 &
p2_pid=$!
s3_rc=0; wait "$s3_pid" || s3_rc=$?
p2_rc=0; wait "$p2_pid" || p2_rc=$?
[ "$s3_rc" -eq 0 ] && [ "$p2_rc" -eq 0 ]

# Verify fixed-point identity for both serial and parallel compilers using the
# newly built 4-worker compiler so stage3==stage4 and serial-produced==four-workers
# execute concurrently.
if ! cmp -s "$work/stage2" "$work/stage3"; then
    "$work/stage2-par" build "selfhost/$entry.ddr" --jobs 4 -o "$work/stage4" >&2 &
    fp_pid=$!
    "$work/stage2-par" build selfhost/main_parallel.ddr --jobs 4 -o "$work/four-workers" >&2 &
    p4_pid=$!
    fp_rc=0; wait "$fp_pid" || fp_rc=$?
    p4_rc=0; wait "$p4_pid" || p4_rc=$?
    [ "$fp_rc" -eq 0 ] && [ "$p4_rc" -eq 0 ]
    cmp "$work/stage3" "$work/stage4"
    cmp "$work/stage2-par" "$work/four-workers"
else
    "$work/stage2-par" build selfhost/main_parallel.ddr --jobs 4 -o "$work/four-workers" >&2
    cmp "$work/stage2-par" "$work/four-workers"
fi

mv -f "$work/stage3" "$dir/dunder-serial"
cp "$work/four-workers" "$work/parallel-pub"
mv -f "$work/parallel-pub" "$dir/dunder-parallel"
mv -f "$work/four-workers" "$dir/dunder"
printf '%s\n' "$root/$dir/dunder"
