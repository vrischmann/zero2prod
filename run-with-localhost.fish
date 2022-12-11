#!/usr/bin/env fish

set -x DATABASE_NAME zero2prod
set -x RUST_LOG sqlx=error,info

cargo run | bunyan
