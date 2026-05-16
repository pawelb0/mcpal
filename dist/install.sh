#!/usr/bin/env bash
# Install the latest mcpal release binary into $HOME/.local/bin (or
# $MCPAL_INSTALL_DIR, if set). Pulls from GitHub Releases artifacts
# built by .github/workflows/release.yml.
#
# Usage:
#   curl --proto '=https' --tlsv1.2 -fsSL \
#     https://raw.githubusercontent.com/pawelb0/mcpal/main/dist/install.sh | sh
#
# Env:
#   MCPAL_VERSION=v0.1.0          pin a release; default is the latest tag
#   MCPAL_INSTALL_DIR=$HOME/bin   override install directory
set -euo pipefail

repo="pawelb0/mcpal"
version="${MCPAL_VERSION:-}"
install_dir="${MCPAL_INSTALL_DIR:-$HOME/.local/bin}"

if [ -z "$version" ]; then
    version=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" \
        | grep -m1 '"tag_name"' \
        | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
fi
if [ -z "$version" ]; then
    echo "install.sh: could not resolve a release tag for $repo" >&2
    exit 1
fi

uname_s=$(uname -s)
uname_m=$(uname -m)
case "$uname_s-$uname_m" in
    Darwin-arm64)    target="aarch64-apple-darwin" ;;
    Darwin-x86_64)   target="x86_64-apple-darwin" ;;
    Linux-x86_64)    target="x86_64-unknown-linux-gnu" ;;
    *)
        echo "install.sh: no prebuilt binary for $uname_s-$uname_m" >&2
        echo "install.sh: cargo install --git https://github.com/$repo --path crates/mcpal" >&2
        exit 1
        ;;
esac

archive="mcpal-$version-$target.tar.gz"
url="https://github.com/$repo/releases/download/$version/$archive"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "install.sh: downloading $url" >&2
curl -fL --proto '=https' --tlsv1.2 -o "$tmp/$archive" "$url"
tar -xzf "$tmp/$archive" -C "$tmp"

mkdir -p "$install_dir"
install -m 0755 "$tmp/mcpal" "$install_dir/mcpal"

echo "install.sh: installed $install_dir/mcpal ($version)" >&2
case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) echo "install.sh: $install_dir is not on \$PATH; add it to your shell rc" >&2 ;;
esac
