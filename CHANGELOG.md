# Changelog

## [0.3.0] - 2026-09-21

### Anadido
- Token gestionable por prioridad: CLI, variable de entorno, config o fichero 0600.
- `scripts/token.sh` (generate/show/clear) y generacion automatica en la instalacion.
- Interruptor del panel: `panel: true|false` y `scripts/panel.sh on|off|status`.
- `deploy/install.sh --no-enable` para instalar sin arranque automatico.

### Cambiado
- El panel solo exige token cuando hay uno configurado; avisa si se expone en red sin token.

## [0.2.0] - 2026-09-21

### Anadido
- Panel web embebido (axum) con eventos en vivo por SSE y contadores.
- API HTTP de control: politica, bloqueos, enforce/monitor y apagado.
- Configuracion por YAML (`config/agent.yaml`) y politica persistente.
- Interruptor de enforcement en eBPF (mapa CONTROL) para pausar sin desenganchar.
- Autenticacion opcional por token (cabecera Bearer o `?token=`).
- Auto-proteccion: el loopback no se puede bloquear.
- Despliegue como servicio systemd (`deploy/install.sh`).
- Log de eventos en JSONL.

### Cambiado
- El agente de usuario se reestructura en actor + API + config + policy + state.

## [0.1.0] - 2026-09-21

### Anadido
- Primer MVP: hook eBPF cgroup/connect4 con LPM trie (CIDR) y ring buffer.
- CLI con `--block`, `--cgroup` y `--log-allowed`.
- Prueba real: bloqueo inmediato (0 ms) y desbloqueo al salir del proceso.
