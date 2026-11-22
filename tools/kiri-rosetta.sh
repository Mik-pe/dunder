#!/bin/sh
set -eu

case "$1" in
    --open)
        shift
        case "$(readlink /proc/1/exe)" in
            */rosetta) ;;
            *) echo 'Kiri x86-64 does not preserve argv[0] and its interpreter is not Rosetta.' >&2; exit 1 ;;
        esac
        parent_ns=$(readlink /proc/self/ns/user)
        exec 3</proc/1/exe
        exec unshare --user --map-root-user --mount /bin/sh "$0" --register "$parent_ns" "$@"
        ;;
    --register)
        parent_ns=$2
        shift 2
        [ "$(readlink /proc/self/ns/user)" != "$parent_ns" ] || exit 1
        directory=$(mktemp -d)
        mount -t binfmt_misc binfmt_misc "$directory"
        for entry in "$directory"/*; do
            case "$entry" in
                "$directory/register"|"$directory/status") ;;
                *) echo 'Kiri needs a fresh, private binfmt_misc table for Rosetta.' >&2; exit 1 ;;
            esac
        done
        magic='\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\x3e\x00'
        mask='\xff\xff\xff\xff\xff\xfe\xfe\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff'
        printf '%s\n' ":dunder-x86:M::$magic:$mask:/proc/self/fd/3:PF" > "$directory/register"
        exec 3<&-
        mount -o remount,ro "$directory"
        exec setpriv --no-new-privs --bounding-set=-all --inh-caps=-all --ambient-caps=-all "$@"
        ;;
    *)
        # A child can open PID 1's actual Rosetta executable. Rosetta virtualizes /proc/self/exe.
        trap 'exit 143' TERM
        /bin/sh "$0" --open "$@" &
        wait "$!"
        ;;
esac
