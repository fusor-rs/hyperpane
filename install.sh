#!/bin/sh
set -eu

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

release_target() {
  [ "$(uname -s)" = Darwin ] || fail "hyperpane requires macOS with Xcode"
  case "$(uname -m)" in
    arm64 | aarch64) printf 'aarch64-apple-darwin\n' ;;
    *) fail "hyperpane requires a Mac with Apple silicon" ;;
  esac
}

verify_archive() {
  actual=$(shasum -a 256 "$1")
  expected=$(cut -d ' ' -f 1 "$1.sha256")
  [ "$expected" = "${actual%% *}" ] || fail "checksum mismatch; download the release again"
}

release_version() {
  version=${1:-}
  if [ -z "$version" ]; then
    latest=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$releases/latest") ||
      fail "cannot find the latest release; check $releases or specify a version"
    case "$latest" in
      "$releases"/tag/v*) version=${latest##*/} ;;
      *) fail "no release found; check $releases" ;;
    esac
  fi
  version=${version#v}
  case "$version" in
    '' | *[!0-9A-Za-z.+-]*) fail "invalid version; use a release version such as v0.1.0" ;;
  esac
  printf '%s\n' "$version"
}

main() {
  action=${1:-}
  if [ "$action" = --archive ]; then
    [ "$#" -eq 2 ] || fail "usage: sh install.sh --archive VERSION"
    shift
  fi
  [ "$#" -le 1 ] || fail "usage: sh install.sh [VERSION]"
  releases="https://github.com/fusor-rs/hyperpane/releases"
  target=$(release_target)
  version=$(release_version "${1:-}")
  name="hyperpane-$version-$target"
  archive="$name.tar.gz"
  if [ "$action" = --archive ]; then
    printf '%s\n' "$archive"
    return
  fi
  for tool in curl tar shasum; do
    command -v "$tool" >/dev/null 2>&1 || fail "install $tool before running this installer"
  done
  url="${HYPERPANE_DOWNLOAD_BASE:-$releases/download}/v$version/$archive"
  root=${HYPERPANE_INSTALL:-$HOME/.hyperpane}
  umask 022
  mkdir -p "$root/bin" "$root/hosts"
  temporary=$(mktemp -d "$root/.install.XXXXXX")
  trap 'rm -r "$temporary"' 0
  trap 'exit 1' HUP INT TERM

  printf 'Downloading hyperpane v%s for %s\n' "$version" "$target"
  curl -fsSL "$url" -o "$temporary/$archive" || fail "cannot download $url"
  curl -fsSL "$url.sha256" -o "$temporary/$archive.sha256" ||
    fail "cannot download $url.sha256"
  verify_archive "$temporary/$archive"
  tar -xzf "$temporary/$archive" -C "$temporary"
  unpacked="$temporary/$name"
  for file in bin/hyperpane "hosts/$target/hyperpane-host" hosts/aarch64-apple-ios-sim/hyperpane-host; do
    [ -f "$unpacked/$file" ] && [ ! -L "$unpacked/$file" ] ||
      fail "archive has no regular $file"
  done
  installed_version=$("$unpacked/bin/hyperpane" --version) ||
    fail "downloaded hyperpane could not run"
  [ "$installed_version" = "hyperpane $version" ] ||
    fail "downloaded hyperpane has the wrong version"
  for host in "$unpacked"/hosts/*; do
    rm -rf "$root/hosts/${host##*/}"
    mv "$host" "$root/hosts/"
  done
  mv -f "$unpacked/bin/hyperpane" "$root/bin/hyperpane"
  printf 'Installed hyperpane v%s to %s\n' "$version" "$root"
  case ":$PATH:" in
    *":$root/bin:"*) ;;
    *) printf 'Add %s to PATH in your shell profile.\n' "$root/bin" ;;
  esac
}

main "$@"
