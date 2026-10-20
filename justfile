# The fusor CLI from the sibling checkout, so the consumer's crates and the
# CLI that builds it always come from the same source.
fusor := "cargo run --manifest-path ../rust-front/Cargo.toml --locked --quiet -p fusor-cli --bin fusor --"
fusor_bin := justfile_directory() / "../rust-front/target/debug/fusor"

default:
    @just --list

check: lint msrv consumer consumer-ios

lint:
    sh -n install.sh
    cargo fmt --all -- --check
    cargo fmt --manifest-path tests/consumer/Cargo.toml -- --check
    cargo test --workspace --locked
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo clippy -p hyperpane-shell --target aarch64-apple-ios-sim --locked -- -D warnings
    cargo check -p hyperpane-shell --target aarch64-apple-ios --locked
    cargo clippy --manifest-path tests/consumer/Cargo.toml --target wasm32-unknown-unknown --locked -- -D warnings
    cargo doc --workspace --no-deps --locked

msrv:
    cargo +1.85 test --workspace --locked
    cargo +1.85 check -p hyperpane-shell --target aarch64-apple-ios-sim --locked

# Downloads the tools fusor builds with (wasm-bindgen and others) once.
setup:
    {{fusor}} install --manifest-path tests/consumer/Cargo.toml --locked

# hyperpane builds the consumer with this fusor CLI through FUSOR_BIN.
fusor-cli:
    cargo build --manifest-path ../rust-front/Cargo.toml --locked --quiet -p fusor-cli --bin fusor

consumer: fusor-cli
    FUSOR_BIN={{fusor_bin}} cargo test -p hyperpane-cli --test consumer --locked -- --exact consumer_runs_in_a_macos_window

consumer-ios: fusor-cli
    FUSOR_BIN={{fusor_bin}} cargo test -p hyperpane-cli --test consumer --locked -- --exact consumer_runs_in_the_ios_simulator

doctor:
    cargo run -p hyperpane-cli --locked -- doctor

# The release archive for Apple silicon: the CLI and the prebuilt hosts it
# ships, in target/release-archives/v<version>/.
archive:
    #!/usr/bin/env bash
    set -euo pipefail
    target=aarch64-apple-darwin
    version=$(cargo pkgid -p hyperpane-cli)
    version=${version##*#}
    cargo build --locked --release --target "$target" --bin hyperpane --bin hyperpane-host
    # Bundling builds the iOS Simulator host the way the CLI always does.
    work=$(mktemp -d)
    trap 'rm -r "$work"' EXIT
    mkdir "$work/site"
    printf '{"version":1,"generation":"g-0","base_path":"/"}' > "$work/site/.fusor-output.json"
    printf '<!doctype html><title>Release</title>\n' > "$work/site/index.html"
    "target/$target/release/hyperpane" bundle --ios --dist "$work/site" --name Release --out "$work"
    name="hyperpane-$version-$target"
    staging="target/release-archives/$name"
    out="target/release-archives/v$version"
    rm -rf "$staging"
    mkdir -p "$staging/bin" "$staging/hosts/$target" "$staging/hosts/aarch64-apple-ios-sim" "$out"
    cp "target/$target/release/hyperpane" "$staging/bin/"
    cp "target/$target/release/hyperpane-host" "$staging/hosts/$target/"
    cp target/aarch64-apple-ios-sim/release/hyperpane-host "$staging/hosts/aarch64-apple-ios-sim/"
    cp LICENSE README.md "$staging/"
    COPYFILE_DISABLE=1 tar -czf "$out/$name.tar.gz" -C target/release-archives "$name"
    (cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
    echo "$out/$name.tar.gz"

# Installs the archive from `just archive` with install.sh and checks that the
# installed CLI bundles its shipped host and that a corrupt download is refused.
archive-check:
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(cargo pkgid -p hyperpane-cli)
    version=${version##*#}
    archives="$PWD/target/release-archives"
    archive="$archives/v$version/hyperpane-$version-aarch64-apple-darwin.tar.gz"
    work=$(mktemp -d)
    trap 'rm -r "$work"' EXIT
    export HYPERPANE_DOWNLOAD_BASE="file://$archives"
    export HYPERPANE_INSTALL="$work/installed hyperpane"
    sh install.sh "v$version"
    test "$("$HYPERPANE_INSTALL/bin/hyperpane" --version)" = "hyperpane $version"
    mkdir "$work/site"
    printf '{"version":1,"generation":"g-0","base_path":"/"}' > "$work/site/.fusor-output.json"
    printf '<!doctype html><title>Release</title>\n' > "$work/site/index.html"
    "$HYPERPANE_INSTALL/bin/hyperpane" bundle --ios --dist "$work/site" --name Installed --out "$work" 2> "$work/bundle.log"
    cat "$work/bundle.log"
    if grep -F "Building hyperpane-host" "$work/bundle.log"; then
        echo "the installed CLI built a host instead of using the shipped one" >&2
        exit 1
    fi
    cmp "$work/Installed.app/hyperpane-host" "$HYPERPANE_INSTALL/hosts/aarch64-apple-ios-sim/hyperpane-host"
    cp "$archive" "$work/intact.tar.gz"
    printf 'corrupt download' >> "$archive"
    if sh install.sh "v$version" > "$work/rejected.log" 2>&1; then
        mv "$work/intact.tar.gz" "$archive"
        echo "install.sh accepted a corrupt archive" >&2
        exit 1
    fi
    mv "$work/intact.tar.gz" "$archive"
    grep -Fx 'error: checksum mismatch; download the release again' "$work/rejected.log"

# Upgrades an install.sh installation of this version to the next patch
# version, both built from this checkout and served from local files.
upgrade-check: archive
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(cargo pkgid -p hyperpane-cli)
    version=${version##*#}
    next=$(echo "$version" | awk -F. '{ printf "%d.%d.%d", $1, $2, $3 + 1 }')
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    rsync -a --exclude target --exclude .git --exclude tests/consumer ./ "$work/next/"
    sed -i '' "s/^version = \"$version\"/version = \"$next\"/; s/\"=$version\"/\"=$next\"/" "$work/next/Cargo.toml"
    cargo update --manifest-path "$work/next/Cargo.toml" --workspace --offline --quiet
    (cd "$work/next" && just archive)
    archives="$work/next/target/release-archives"
    cp -R "target/release-archives/v$version" "$archives/"
    archive=$(sh install.sh --archive "$next")
    release() {
        printf '{"tag_name":"v%s","draft":false,"prerelease":false,"assets":[%s]}' "$next" "$1" > "$work/release.json"
    }
    export HYPERPANE_DOWNLOAD_BASE="file://$archives"
    export HYPERPANE_INSTALL="$work/installed"
    export HYPERPANE_RELEASE_URL="file://$work/release.json"
    installed="$HYPERPANE_INSTALL/bin/hyperpane"
    sh install.sh "v$version"
    release '{"name":"'"$archive"'"},{"name":"'"$archive"'.sha256"}'
    if "$installed" upgrade 2> "$work/incomplete.log"; then
        echo "upgrade accepted a release without install.sh" >&2
        exit 1
    fi
    grep -F "is still being published" "$work/incomplete.log"
    test "$("$installed" --version)" = "hyperpane $version"
    release '{"name":"install.sh"},{"name":"'"$archive"'"},{"name":"'"$archive"'.sha256"}'
    "$installed" upgrade
    test "$("$installed" --version)" = "hyperpane $next"
    cmp "$HYPERPANE_INSTALL/hosts/aarch64-apple-ios-sim/hyperpane-host" \
        "$archives/hyperpane-$next-aarch64-apple-darwin/hosts/aarch64-apple-ios-sim/hyperpane-host"
    "$installed" upgrade 2>&1 | grep -F "hyperpane $next is the latest release"
