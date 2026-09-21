# Plan: filtrado de egreso real (allowlist por dominio y proceso)

> Documento de trabajo para continuar. Fecha objetivo de la sesion: 2026-09-22.
> Estado actual: fase 1 y 2 hechas (bloqueo por IP + panel/servicio). Esto es la fase 3 del enforcement.

## Objetivo

Convertir soar-agent en una herramienta de **filtrado de egreso** desplegable:
**denegar por defecto** y permitir solo lo autorizado, atando la politica al **dominio** y a la
**identidad del proceso**, no al numero IP.

## Por que (datos reales de esta maquina)

- 841 eventos -> **117 IPs destino distintas** y **77 procesos distintos** en un rato normal.
- Telegram, AWS y GitHub Pages (8 IPs) rotan direcciones.
- Mantener una allowlist por IP es inviable: se rompe sola y acaba tan abierta que no protege.

Conclusion: la politica debe enlazarse al **nombre (FQDN)** o al **proceso**, no a la IP.

## Modelo de politica objetivo

    # /etc/soar-agent/policy.yaml
    default: deny            # deny = filtrado de egreso real; allow = comportamiento actual

    allow:
      processes:
        - path: /usr/bin/node
          note: "dsh web"
        - path: /usr/bin/python3
          note: "bot telegram"
      domains:
        - api.telegram.org
        - update.ubuntu.com
        - github.com
      ips:
        - 10.0.0.0/8          # red interna fija
        - 192.168.0.10/32     # un servidor concreto

    break_glass:
      enabled: true
      ttl_seconds: 300        # reactiva todo temporalmente y caduca solo

## Arquitectura tecnica

1. **Enforcement (facil):** invertir la decision en eBPF. Anadir un mapa ALLOWLIST (LPM trie) y un
   byte de accion por defecto en CONTROL (p.ej. CONTROL[1]: 1 = permitir, 0 = denegar). En connect4:
   si la IP esta en allowlist -> PASS; si no, aplicar la accion por defecto.

2. **Identidad del proceso (estable):** un tracepoint sched_process_exec puebla un mapa
   TGID -> identidad del ejecutable (inode+dev o ruta). En connect4 se consulta por
   bpf_get_current_pid_tgid y se compara con la allowlist de procesos. Alternativa: task_btf +
   bpf_d_path (requiere LSM). Objetivo: "solo este binario puede salir".

3. **Dominio (lo que de verdad se quiere):**
   - Opcion A (empezar aqui): capturar DNS (kprobe en udp_sendmsg/udp_recvmsg o tracepoint de DNS) y
     mantener un mapa IP -> FQDN con TTL. En connect4, si la IP tiene un dominio permitido -> PASS.
     Limitacion: DoH/DoT no pasan por ahi y los CDN comparten IP.
   - Opcion B (enforcement fuerte): proxy transparente con inspeccion de SNI (Envoy o Squid) delante
     del trafico; el agente bloquea por connect y el proxy decide por dominio.
   - Recomendacion: A para visibilidad y correlacion; B cuando haga falta enforcement duro.

4. **soar-agent learn:** subcomando que lee /var/log/soar-agent/events.jsonl, agrupa por proceso y
   por destino, y **propone** policy.suggested.yaml (procesos, dominios e IPs observados). El humano
   revisa y la promueve a allowlist. Es el paso que hace desplegable el default-deny.

## Fases

- [ ] **F1** - policy.default (allow|deny) + allowlist.ips + enforcement invertido + pruebas.
- [ ] **F2** - allowlist por proceso (mapa TGID->exe, learn por proceso).
- [ ] **F3** - soar-agent learn -> propuesta de politica desde el log.
- [ ] **F4** - allowlist por dominio (correlacion DNS + TTL).
- [ ] **F5** - break-glass con TTL + "deny pero solo registrar" para validar sin romper.
- [ ] **F6** - docs + demo (captura del panel) para GitHub/LinkedIn.

## Criterios de aceptacion

- Con default: deny, curl a un destino no autorizado falla, y solo pasan los autorizados.
- soar-agent learn genera un YAML revisable a partir del log real.
- Que un dominio permitido cambie de IP **no rompe** nada.
- Break-glass reactiva todo y caduca solo, sin reiniciar a mano.

## Riesgos y mitigaciones

- **Romper tu propio trabajo:** migrar siempre en modo aprendizaje/monitor; break-glass; allowlist
  obligatoria para la IP de gestion y para el DNS.
- **DoH/DoT escapa** de la correlacion DNS -> documentar y usar SNI en F5.
- **Permitir por proceso sin destino** abre la puerta a ese proceso entero -> combinar proceso+destino.
- **Reducir demasiado** deja de ser util -> la lista debe ser revisable y versionada.

## Referencias

- Cilium / Tetragon (seguridad con eBPF y politica por identidad).
- Kubernetes NetworkPolicy (politica por etiquetas).
- Proxy con SNI (Envoy/Squid) y DNS filtering (CoreDNS/Pi-hole).

## Como retomamos manana

1. Leer este documento y AGENTS.md.
2. Empezar por **F1** (default + allowlist de IP) con pruebas en monitor.
3. Seguir con **F2** (proceso) y **F3** (learn).
