#!/bin/sh
# Rebuild Dunder from the frozen compiler for this host.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
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
"$work/stage2" build "selfhost/$entry.ddr" -o "$work/stage3" >&2
if ! cmp -s "$work/stage2" "$work/stage3"; then
    # A new compiler can intentionally change its own emitted code. Rebuild
    # once more with the new emitter, then require an identical fixed point.
    "$work/stage3" build "selfhost/$entry.ddr" -o "$work/stage4" >&2
    cmp "$work/stage3" "$work/stage4"
fi
mv -f "$work/stage3" "$dir/dunder"
printf '%s\n' "$root/$dir/dunder"
