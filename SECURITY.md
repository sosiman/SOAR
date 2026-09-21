# Seguridad

## Reportar una vulnerabilidad

No abras un issue publico. Envia un correo al mantenedor con:

- Descripcion del problema y su impacto.
- Pasos para reproducirlo.
- Version afectada y entorno (kernel, distribucion).

## Consideraciones de despliegue

- Este agente engancha eBPF a una cgroup: engancharlo a la raiz afecta a todo el sistema.
- Usa el modo monitor para validar la politica antes de bloquear.
- Si expones el panel fuera de localhost, define un token y pon TLS por delante.
- El loopback no se bloquea por diseno, para no perder el acceso al panel.
