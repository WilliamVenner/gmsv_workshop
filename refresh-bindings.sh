#!/bin/bash
cargo check --release --features refresh-bindgen --target x86_64-unknown-linux-gnu
cargo check --release --features refresh-bindgen --target i686-unknown-linux-gnu