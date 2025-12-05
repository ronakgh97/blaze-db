#!/usr/bin/env just --justfile

release:
    cargo build --release    

lint:
    cargo clippy

search:
    cargo run --bin search -- release

load:
    cargo run --bin load -- release

write:
    cargo run --bin write -- release
