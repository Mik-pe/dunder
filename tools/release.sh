#!/bin/sh
# Package a single committed Dunder tree locally; an explicit --publish uploads it.
set -eu

usage() { echo 'usage: tools/release.sh <tag> [--outdir dir] [--publish]'; }
fail() { echo "release: $*" >&2; exit 1; }

tag=
publish=0
outdir=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --publish) publish=1 ;;
        --outdir) [ "$#" -ge 2 ] || { usage >&2; exit 2; }; outdir=$2; shift ;;
        -h|--help) usage; exit 0 ;;
        -*) usage >&2; exit 2 ;;
        *) [ -z "$tag" ] || { usage >&2; exit 2; }; tag=$1 ;;
    esac
    shift
done
case "$tag" in
    ''|*[!a-zA-Z0-9._+-]*) fail 'expected a version tag such as v0.1.0' ;;
esac
git check-ref-format "refs/tags/$tag" || fail "invalid tag $tag"
version=${tag#v}
repo=Mik-pe/dunder
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
outdir=${outdir:-"$root/build/release/$tag"}
mkdir -p "$outdir"
outdir=$(CDPATH='' cd -- "$outdir" && pwd)
work=$(mktemp -d "$outdir/.package.XXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM
cd "$root"

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        digest=$(sha256sum < "$1") || return 1
    else
        digest=$(shasum -a 256 < "$1") || return 1
    fi
    printf '%s\n' "${digest%% *}"
}

# Matching refs returns an empty list for a missing tag. A network or auth error
# must propagate instead of being interpreted as permission to create a tag.
remote_commit() {
    object=$(gh api "repos/$repo/git/matching-refs/tags/$tag" --jq \
        ".[] | select(.ref == \"refs/tags/$tag\") | .object | .type + \" \" + .sha") || return 1
    while [ "${object%% *}" = tag ]; do
        object=$(gh api "repos/$repo/git/tags/${object#* }" --jq '.object | .type + " " + .sha') || return 1
    done
    case "$object" in
        '') ;;
        'commit '*) printf '%s\n' "${object#* }" ;;
        *) fail "tag $tag does not name a commit" ;;
    esac
}

local_object=$(git rev-parse --quiet --verify "refs/tags/$tag") || local_object=
local_tag=
if [ -n "$local_object" ]; then
    local_tag=$(git rev-parse --verify "$local_object^{commit}") || fail "tag $tag does not name a commit"
fi
commit=${local_tag:-$(git rev-parse --verify HEAD)}
remote_tag=
if [ "$publish" -eq 1 ]; then
    remote_tag=$(remote_commit)
    if [ -n "$remote_tag" ]; then
        [ -z "$local_tag" ] || [ "$local_tag" = "$remote_tag" ] || fail "local and GitHub tags differ for $tag"
        commit=$remote_tag
        git cat-file -e "$commit^{commit}" 2>/dev/null || git fetch --no-tags --no-write-fetch-head "https://github.com/$repo.git" "refs/tags/$tag"
    fi
fi

mkdir "$work/source" "$work/images" "$work/assets"
git archive --format=tar --output="$work/source.tar" "$commit" bootstrap selfhost stdlib runtime tools/bootstrap.sh tools/bootstrap-parallel.sh
tar -xf "$work/source.tar" -C "$work/source"
cd "$work/source"
compiler=$(./tools/bootstrap.sh)
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) host=arm64-macos ;;
    Linux:aarch64|Linux:arm64) host=arm64-linux ;;
    Linux:x86_64) host=x86_64-linux ;;
    *) fail 'unsupported release host' ;;
esac
reported=$("$compiler" --version)
[ "$reported" = "dunder $version ($host)" ] || fail "tag $tag does not match compiler: $reported"

manifest="$work/assets/release.txt"
printf 'version=%s\ncommit=%s\n' "$version" "$commit" > "$manifest"
for target in arm64-macos arm64-linux x86_64-linux; do
    image="$work/images/$target"
    if [ "$host" = "$target" ]; then
        cp "$compiler" "$image"
    else
        case "$target" in
            arm64-macos) entry=main_parallel ;;
            arm64-linux) entry=linux ;;
            x86_64-linux) entry=x86_linux ;;
        esac
        "$compiler" build "selfhost/$entry.ddr" --target "$target" -o "$image"
    fi
    archive="$work/assets/$target.gz"
    gzip -9 -n -c "$image" > "$archive"
    compressed=$(sha256_of "$archive")
    raw=$(sha256_of "$image")
    printf '%s  %s.gz\n' "$compressed" "$target" > "$work/assets/$target.sha256"
    printf '%s.gz.sha256=%s\n%s.sha256=%s\n' "$target" "$compressed" "$target" "$raw" >> "$manifest"
done

# Complete all builds before replacing any local asset. Readers can validate
# against release.txt, which is installed last. Upload from this run's snapshot.
cd "$work/assets"
for file in *.gz *.sha256 release.txt; do
    cp "$file" "$work/$file"
    mv -f "$work/$file" "$outdir/$file"
done
printf 'Packaged Dunder %s (%s) in %s\n' "$version" "$commit" "$outdir"
[ "$publish" -eq 1 ] || exit 0

if [ -z "$remote_tag" ]; then
    # Create the release tag at the exact commit the local bootstrap consumed.
    # Preserve the annotated tag object when it is already present locally.
    git -C "$root" push "https://github.com/$repo.git" "${local_object:-$commit}:refs/tags/$tag"
fi
[ "$(remote_commit)" = "$commit" ] || fail "GitHub tag $tag does not point to $commit"

# Include drafts, and distinguish a missing release from a failed API request.
state=$(gh api --paginate "repos/$repo/releases?per_page=100" \
    --jq ".[] | select(.tag_name == \"$tag\") | if .draft then \"draft\" else \"published\" end")
if [ -z "$state" ]; then
    gh release create "$tag" --repo "$repo" --verify-tag --target "$commit" --draft \
        --title "Dunder $tag" --notes "Local compiler builds for arm64-macos, arm64-linux and x86_64-linux."
    state=draft
fi
if [ "$state" = draft ]; then
    # GitHub's release asset digests let an interrupted run skip correct files
    # and repair only missing or incomplete assets, while the release is hidden.
    existing=$(gh release view "$tag" --repo "$repo" --json assets \
        --jq '.assets[] | .name + " " + (.digest // "")')
    for file in *.gz *.sha256 release.txt; do
        digest=$(sha256_of "$file")
        previous=$(printf '%s\n' "$existing" | awk -v name="$file" '$1 == name {print ($2 == "" ? "unknown" : $2)}')
        [ "$previous" != "sha256:$digest" ] || continue
        if [ -n "$previous" ]; then
            # An interrupted upload may leave a zero-byte asset in a draft.
            is_draft=$(gh release view "$tag" --repo "$repo" --json isDraft --jq '.isDraft')
            [ "$is_draft" = true ] || fail "release $tag was published during upload"
            gh release upload "$tag" "$file" --repo "$repo" --clobber
        else
            gh release upload "$tag" "$file" --repo "$repo"
        fi
    done
fi
# Authenticate and compare the downloadable bytes even on a published rerun.
# A published release is never edited to repair mismatched artifacts.
mkdir "$work/download"
set --
for file in *.gz *.sha256 release.txt; do set -- "$@" --pattern "$file"; done
gh release download "$tag" --repo "$repo" --dir "$work/download" "$@"
for file in *.gz *.sha256 release.txt; do
    cmp -s "$file" "$work/download/$file" || fail "downloaded release asset differs: $file"
done
if [ "$state" = draft ]; then
    case "$version" in
        *-*) gh release edit "$tag" --repo "$repo" --verify-tag --draft=false --prerelease ;;
        *)   gh release edit "$tag" --repo "$repo" --verify-tag --draft=false ;;
    esac
fi
printf 'Verified https://github.com/%s/releases/tag/%s\n' "$repo" "$tag"
