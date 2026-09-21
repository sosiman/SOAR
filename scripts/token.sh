#!/usr/bin/env bash
# Gestiona el token del panel: generate | show | clear.
set -euo pipefail
arg="$1"
[ -z "$arg" ] && arg=show
CONF=/etc/soar-agent/agent.yaml
TOK=/etc/soar-agent/token

case "$arg" in
  generate)
    new="$(head -c 32 /dev/urandom | base64 | tr -d '/+=' | cut -c1-40)"
    printf '%s\n' "$new" | sudo tee "$TOK" >/dev/null
    sudo chmod 600 "$TOK"
    sudo chown root:root "$TOK"
    if sudo grep -q '^token_file:' "$CONF"; then
      sudo sed -i "s#^token_file:.*#token_file: \"$TOK\"#" "$CONF"
    else
      printf 'token_file: "%s"\n' "$TOK" | sudo tee -a "$CONF" >/dev/null
    fi
    echo "Token generado y configurado en $CONF"
    echo "Token: $new"
    if systemctl is-active soar-agent >/dev/null 2>&1; then
      sudo systemctl restart soar-agent
      echo "servicio reiniciado"
    fi
    ;;
  show)
    sudo cat "$TOK" 2>/dev/null || echo "no hay token en $TOK"
    ;;
  clear)
    sudo rm -f "$TOK"
    echo "Token eliminado. Quita token_file de $CONF y reinicia para dejarlo sin autenticacion."
    ;;
  *)
    echo "uso: $0 generate|show|clear"; exit 1;;
esac
