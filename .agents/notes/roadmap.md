# Roadmap

## Fase 1 (HECHA) - Enforcement y observabilidad

- [x] Hook cgroup/connect4 con LPM trie (CIDR) y ring buffer de eventos.
- [x] CLI: --block (repetible), --cgroup, --log-allowed.
- [x] Prueba real: 1.1.1.1 bloqueado en 0 ms; example.com 200; desbloqueo al salir.

## Fase 2 - Politica declarativa y modo daemon

- [ ] Fichero de politica YAML (allowlist + blocklist + solo-log) y verificacion de firma.
- [ ] Modo daemon: recarga en caliente de la politica sin reenganchar el hook.
- [ ] Metricas basicas (conexiones permitidas/bloqueadas) y endpoint local de salud.
- [ ] Soporte IPv6 (connect6) y UDP (sendmsg4/6).
- [ ] Logging estructurado (JSON) en vez de println.
- Criterio de aceptacion: cambiar el YAML y ver el efecto sin reiniciar el proceso.

## Fase 3 - Orquestacion SOAR

- [ ] Publicar eventos a un bus (NATS) ademas de stdout.
- [ ] Orquestador en Go: motor de playbooks trigger -> condicion -> accion.
- [ ] Conectores (webhook, ticket, SIEM) y gestion de casos en PostgreSQL.
- [ ] Idempotencia, reintentos y auditoria de cada accion con id de playbook.
- Criterio de aceptacion: un playbook que, ante una alerta, llame al agente para bloquear una IP.

## Fase 4 - Mas acciones y respuesta

- [ ] Matar proceso (pid + cgroup, con allowlist de procesos criticos).
- [ ] Cuarentena de fichero con fanotify (FAN_OPEN_PERM).
- [ ] Contencion de usuario (seccomp/unshare).
- [ ] Recoleccion forense (hashes, /proc) antes de actuar.
- [ ] Reversibilidad: toda accion con su comando de deshacer o TTL.
- Criterio de aceptacion: cada accion es auditable y reversible o caduca sola.

## Deuda tecnica conocida

- Solo IPv4 y connect(); sin IPv6, sin UDP sendmsg.
- El estado (blocklist) se reconstruye al arrancar; aun no se persiste.
- Sin tests automatizados: la verificacion es la prueba manual con curl.
- La lista de bloqueo no se reconcilia si el mapa se llena (4096 entradas).
