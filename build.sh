#!/bin/sh

RUSTFLAGS="$RUSTFLAGS -A unused_variables -A dead_code -A unused_parens" cargo build
#cargo build --release
