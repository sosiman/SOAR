#!/usr/bin/env bash
# Instala soar-agent como servicio systemd (autonomo, arranca solo).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> Compilando (release)"
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release

echo "==> Instalando binario en /usr/local/bin"
sudo install -m 0755 target/release/soar-agent /usr/local/bin/soar-agent

echo "==> Creando /etc/soar-agent y /var/log/soar-agent"
sudo mkdir -p /etc/soar-agent /var/log/soar-agent
if [ ! -f /etc/soar-agent/agent.yaml ]; then
  sudo cp config/agent.yaml /etc/soar-agent/agent.yaml
fi
if [ ! -f /etc/soar-agent/policy.yaml ]; then
  sudo cp config/policy.yaml /etc/soar-agent/policy.yaml
fi

echo "==> Instalando unidad systemd"
sudo install -m 0644 deploy/soar-agent.service /etc/systemd/system/soar-agent.service
sudo systemctl daemon-reload
sudo systemctl enable --now soar-agent

echo
echo "Servicio activo. Comprueba con:"
echo "  systemctl status soar-agent"
echo "  journalctl -u soar-agent -f"
echo "  abre http://127.0.0.1:8787 en el navegador"
