#!/bin/sh
# Build Dunder from the frozen seed and install it into $DUNDER_HOME/bin/dunder.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

serial=0
for arg in "$@"; do
    case "$arg" in
        --serial) serial=1 ;;
        -h|--help)
            echo 'usage: tools/install.sh [--serial]' >&2
            exit 0
            ;;
        *)
            echo 'usage: tools/install.sh [--serial]' >&2
            exit 2
            ;;
    esac
done

dunder_home=${DUNDER_HOME:-"${HOME:?HOME must be set}/.dunder"}
bin_dir="$dunder_home/bin"
mkdir -p "$bin_dir"

if [ "$serial" -eq 1 ]; then
    built=$(./tools/bootstrap.sh --serial)
else
    built=$(./tools/bootstrap.sh)
fi

stage=$(mktemp "$bin_dir/.dunder.XXXXXX")
trap 'rm -f "$stage"' EXIT
trap 'exit 1' HUP INT TERM
cp "$built" "$stage"
chmod 755 "$stage"
mv -f "$stage" "$bin_dir/dunder"
trap - EXIT

case ":${PATH:-}:" in
    *":$bin_dir:"*) ;;
    *)
        printf 'Installed %s\n' "$bin_dir/dunder" >&2
        printf 'Add to PATH:\n  export PATH="%s:$PATH"\n' "$bin_dir" >&2
        ;;
esac

printf '%s\n' "$bin_dir/dunder"
