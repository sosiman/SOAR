#!/usr/bin/env bash
# Activa o desactiva el panel web de soar-agent.
# Uso: scripts/panel.sh on|off|status
set -euo pipefail
arg="$1"
[ -z "$arg" ] && arg=status
CONF=/etc/soar-agent/agent.yaml

case "$arg" in
  on|off)
    if [ "$arg" = on ]; then val=true; else val=false; fi
    if [ ! -f "$CONF" ]; then echo "No existe $CONF"; exit 1; fi
    if sudo grep -q '^panel:' "$CONF"; then
      sudo sed -i "s/^panel:.*/panel: $val/" "$CONF"
    else
      echo "panel: $val" | sudo tee -a "$CONF" >/dev/null
    fi
    echo "panel = $val en $CONF"
    if systemctl list-unit-files 2>/dev/null | grep -q '^soar-agent.service'; then
      sudo systemctl restart soar-agent
      # esperar a que el servicio este activo (evita carreras si se pulsa on/off seguido)
      for i in 1 2 3 4 5 6 7 8 9 10; do
        systemctl is-active --quiet soar-agent && break
        sleep 1
      done
      if [ "$val" = true ]; then
        for i in 1 2 3 4 5; do
          ss -ltn 2>/dev/null | grep -q 8787 && break
          sleep 1
        done
        if ss -ltn 2>/dev/null | grep -q 8787; then
          echo "panel escuchando en 8787"
        else
          echo "ojo: el panel no parece escuchar en 8787; revisa journalctl -u soar-agent"
        fi
      else
        echo "panel detenido (el enforcement sigue activo)"
      fi
    fi
    ;;
  status)
    sudo grep '^panel:' "$CONF" 2>/dev/null || echo "panel: (por defecto true)"
    ;;
  *)
    echo "uso: $0 on|off|status"; exit 1;;
esac
