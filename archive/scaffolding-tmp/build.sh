#!/bin/bash
export PATH=$PATH:/root/.cargo/bin:/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin
cd /opt/lan-link
cargo build --release -p lan-linkd 2>&1 | tail -30