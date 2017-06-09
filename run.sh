#!/bin/sh
RUSTFLAGS="$RUSTFLAGS -A unused_variables -A dead_code -A unused_parens" cargo run
#cargo run --release
