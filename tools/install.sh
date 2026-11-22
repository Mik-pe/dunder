#!/bin/sh
set -eu

usage() { echo 'usage: install.sh [--serial | --release [TAG]]'; }
fail() { echo "install: $*" >&2; exit 1; }

case "${1:-}" in
    --release) [ "$#" -le 2 ] || { usage >&2; exit 2; } ;;
    --serial) [ "$#" -eq 1 ] || { usage >&2; exit 2; } ;;
    '') [ "$#" -eq 0 ] || { usage >&2; exit 2; } ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
esac

dunder_home=${DUNDER_HOME:-"${HOME:?HOME must be set}/.dunder"}
mkdir -p "$dunder_home/bin"
bin_dir=$(CDPATH= cd -- "$dunder_home/bin" && pwd)
program=dunder
stage=$(mktemp -d "$bin_dir/.install.XXXXXX")
trap 'rm -rf "$stage"' EXIT
trap 'exit 1' HUP INT TERM

if [ "${1:-}" = --release ]; then
    case "$(uname -s):$(uname -m)" in
        Darwin:arm64) host=arm64-macos ;;
        Linux:aarch64|Linux:arm64) host=arm64-linux ;;
        Linux:x86_64) host=x86_64-linux ;;
        MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64) host=x86_64-windows; program=dunder.exe ;;
        *) fail "no Dunder compiler release for $(uname -sm)" ;;
    esac
    releases=${DUNDER_RELEASES_URL:-https://github.com/Mik-pe/dunder/releases}
    base=${releases%/}/latest/download
    if [ "$#" -eq 2 ]; then
        case "$2" in ''|.|..|-*|*[!a-zA-Z0-9._+-]*) fail 'invalid release tag' ;; esac
        base=${releases%/}/download/$2
    fi
    curl -q -gfsSL --max-time 30 -o "$stage/release.txt" -- "$base/release.txt"
    manifest_value() { awk -v key="$1=" 'index($0, key) == 1 { print substr($0, length(key)+1) }' "$stage/release.txt"; }
    version=$(manifest_value version)
    case "$version" in ''|*[!a-zA-Z0-9._+-]*) fail 'invalid release version' ;; esac
    if [ "$#" -eq 2 ] && [ "${2#v}" != "$version" ]; then fail "release manifest does not match $2"; fi
    verify() {
        if command -v sha256sum >/dev/null 2>&1; then
            actual=$(sha256sum < "$1")
        else
            actual=$(shasum -a 256 < "$1")
        fi
        [ "${actual%% *}" = "$(manifest_value "$2")" ] || fail "SHA-256 mismatch for $2"
    }
    if [ "$program" = dunder.exe ]; then
        curl -q -gfsSL --max-time 120 -o "$stage/$program" -- "$base/$host.exe"
    else
        curl -q -gfsSL --max-time 120 -o "$stage/dunder.gz" -- "$base/$host.gz"
        verify "$stage/dunder.gz" "$host.gz.sha256"
        gzip -dc "$stage/dunder.gz" > "$stage/$program"
    fi
    verify "$stage/$program" "$host.sha256"
    chmod 755 "$stage/$program"
    reported=$("$stage/$program" --version) || fail 'could not run downloaded compiler'
    [ "$reported" = "dunder $version ($host)" ] || fail 'compiler failed its version check'
else
    root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
    built=$("$root/tools/bootstrap.sh" "$@")
    cp "$built" "$stage/dunder"
    chmod 755 "$stage/dunder"
fi
[ ! -d "$bin_dir/$program" ] || fail "$bin_dir/$program is a directory"
mv -f "$stage/$program" "$bin_dir/$program"

case ":${PATH:-}:" in
    *":$bin_dir:"*) ;;
    *)
        printf 'Installed %s\n' "$bin_dir/$program" >&2
        printf 'Add to PATH:\n  export PATH="%s:$PATH"\n' "$bin_dir" >&2
        ;;
esac
printf '%s\n' "$bin_dir/$program"
