#!/usr/bin/env bash
# Compila el agente (arrastra la compilacion del programa eBPF con nightly + bpf-linker).
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
cargo build --release
echo
echo "Binario listo: $(pwd)/target/release/soar-agent"
echo
echo "OJO: compilar NO arranca el agente ni el panel."
echo "  - Servicio autonomo (recomendado):  sudo deploy/install.sh"
echo "  - Arranque manual (necesita root):   scripts/run.sh"
echo "  - Panel:                             http://127.0.0.1:8787"
