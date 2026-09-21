#!/usr/bin/env bash
# Compila el agente (arrastra la compilacion del programa eBPF con nightly + bpf-linker).
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
cargo build --release
echo
echo "Binario listo: $(pwd)/target/release/soar-agent"
