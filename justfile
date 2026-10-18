# The fusor CLI from the sibling checkout, so the consumer's crates and the
# CLI that builds it always come from the same source.
fusor := "cargo run --manifest-path ../rust-front/Cargo.toml --locked --quiet -p fusor-cli --bin fusor --"

default:
    @just --list

check: lint msrv consumer consumer-ios

lint:
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

consumer-app:
    {{fusor}} build --manifest-path tests/consumer/Cargo.toml --locked

consumer: consumer-app
    cargo test -p hyperpane-cli --test consumer --locked -- --exact consumer_runs_in_a_macos_window

consumer-ios: consumer-app
    cargo test -p hyperpane-cli --test consumer --locked -- --exact consumer_runs_in_the_ios_simulator

doctor:
    cargo run -p hyperpane-cli --locked -- doctor
