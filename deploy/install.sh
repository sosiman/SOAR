#!/usr/bin/env bash
# Instala soar-agent como servicio systemd.
# Uso: sudo deploy/install.sh [--no-enable]
#   --no-enable  instala y arranca, pero NO lo activa al encender el PC.
set -euo pipefail
ENABLE=1
for arg in "$@"; do
  if [ "$arg" = "--no-enable" ]; then ENABLE=0; fi
done
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

echo "==> Token del panel (seguridad)"
if [ ! -f /etc/soar-agent/token ]; then
  TOKEN="$(head -c 32 /dev/urandom | base64 | tr -d '/+=' | cut -c1-40)"
  printf '%s\n' "$TOKEN" | sudo tee /etc/soar-agent/token >/dev/null
  sudo chmod 600 /etc/soar-agent/token
  sudo chown root:root /etc/soar-agent/token
  echo "    Token generado: $TOKEN"
fi
if ! sudo grep -q '^token_file:' /etc/soar-agent/agent.yaml; then
  printf 'token_file: "/etc/soar-agent/token"\n' | sudo tee -a /etc/soar-agent/agent.yaml >/dev/null
fi

echo "==> Instalando unidad systemd"
sudo install -m 0644 deploy/soar-agent.service /etc/systemd/system/soar-agent.service
sudo systemctl daemon-reload
if [ "$ENABLE" = 1 ]; then
  sudo systemctl enable --now soar-agent
  echo "==> Activado al arranque (enable)"
else
  sudo systemctl start soar-agent
  echo "==> Arrancado ahora, pero NO se activara al encender (--no-enable)"
fi

echo
echo "Servicio activo. Comprueba con:"
echo "  systemctl status soar-agent"
echo "  journalctl -u soar-agent -f"
echo "  abre http://127.0.0.1:8787 en el navegador"
