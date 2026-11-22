#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
if [ "$#" -ne 0 ]; then
    echo 'usage: tools/bootstrap.sh' >&2
    exit 2
fi
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) host=arm64-macos ;;
    Linux:aarch64|Linux:arm64) host=arm64-linux ;;
    Linux:x86_64) host=x86_64-linux ;;
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
"$work/seed" build selfhost/main.ddr -o "$work/first" >&2
"$work/first" build selfhost/main.ddr -o "$work/second" >&2
if ! cmp -s "$work/first" "$work/second"; then
    "$work/second" build selfhost/main.ddr -o "$work/third" >&2
    cmp "$work/second" "$work/third" >&2
fi
mv -f "$work/second" "$dir/dunder"
printf '%s\n' "$root/$dir/dunder"
