#!/usr/bin/env bash
# Compila (si hace falta) y ejecuta el agente con privilegios.
# Uso: scripts/run.sh --block 1.2.3.4 --block 10.0.0.0/8 [--log-allowed]
set -euo pipefail
cd "$(dirname "$0")/.."
if [ ! -x target/release/soar-agent ]; then
  scripts/build.sh
fi
exec sudo -E ./target/release/soar-agent "$@"
