#!/usr/bin/env just --justfile

release:
    cargo build --release --all-targets

lint:
    cargo clippy
