# soar-agent

> Agente **SOAR** autonomo para Linux: respuesta automatica a ataques con **enforcement eBPF** en el kernel y **panel web** de control.

![license](https://img.shields.io/badge/license-MIT-blue)
![rust](https://img.shields.io/badge/rust-stable%20%2B%20nightly-orange)
![ebpf](https://img.shields.io/badge/eBPF-Aya-blueviolet)
![platform](https://img.shields.io/badge/platform-Linux-lightgrey)

**soar-agent** observa las conexiones salientes de una maquina **dentro del kernel** (eBPF, hook cgroup/connect4) y **bloquea automaticamente** las que coinciden con tu politica. Trae un **panel web** para ver eventos en vivo, editar la politica, pausar el enforcement y detener el servicio. Esta pensado para ejecutarse como **servicio systemd** siempre encendido, sin intervencion manual.

---

## Por que eBPF

El hook se ejecuta **en la ruta de la syscall connect()**, antes de que la conexion se establezca. Devolver 0 la deniega con EPERM de forma **inmediata** (0 ms, no es un timeout) y **dificil de esquivar**. Frente a un modulo de kernel:

- El **verificador** de eBPF garantiza que el programa no puede colgar la maquina.
- **CO-RE**: no hay que recompilar por version de kernel.
- Superficie de ataque minima y sin necesidad de firmar modulos.
- El programa no puede acceder a memoria arbitraria ni a helpers fuera de la lista permitida.

## Caracteristicas

- Enforcement en kernel con hook **cgroup/connect4** (IPv4).
- Lista de bloqueo con soporte **CIDR** (LPM trie).
- **Panel web** embebido: eventos en vivo por SSE, contadores, edicion de politica, pausar/reanudar/parar.
- **API HTTP** para automatizacion e integracion.
- Politica en **YAML** con persistencia y cambios en caliente.
- Interruptor de **enforce/monitor** sin reenganchar el hook.
- **Auto-proteccion**: el loopback no se puede bloquear (no te deja fuera del panel).
- **Log JSONL** de eventos y despliegue como **servicio systemd**.
- Opcional: **token** de acceso al panel.

## Arquitectura

~~~
  (kernel)  cgroup/connect4  -->  LPM trie BLOCKLIST     decision: PASS / DROP
                  |
                  |  RingBuf EVENTS (pid, uid, ip, puerto, comm, blocked)
                  v
  (usuarios)  actor eBPF  --(broadcast)-->  panel web + API HTTP
                  ^
                  |  ordenes (bloquear, pausar, politica)
              panel / API
~~~

- **soar-agent-ebpf**: programa eBPF. LpmTrie (CIDR), Array de control y RingBuf.
- **soar-agent-common**: tipo ConnectEvent compartido (no_std; aya::Pod en usuarios).
- **soar-agent**: actor eBPF, API/panel HTTP (axum) y CLI.

## Requisitos

- Linux con **cgroup v2** y BTF en /sys/kernel/btf/vmlinux.
- Rust **stable** + **nightly** con el componente rust-src.
- clang/LLVM y **bpf-linker**.
- root (para cargar eBPF y enganchar cgroups).

## Inicio rapido

~~~bash
git clone https://github.com/sosiman/SOAR.git
cd SOAR
scripts/setup.sh      # instala toolchain, clang/LLVM y bpf-linker
scripts/build.sh      # compila (arrastra el eBPF con nightly)

sudo ./target/release/soar-agent --listen 127.0.0.1:8787 --monitor
# abre http://127.0.0.1:8787
~~~

Para bloquear de verdad, arranca en modo enforce (por defecto) con una configuracion:

~~~bash
sudo ./target/release/soar-agent --config config/agent.yaml
~~~

## Panel web

El panel se sirve en el puerto indicado por `listen` (por defecto 127.0.0.1:8787). Permite:

- Ver el **estado** (enforce o monitor), uptime y contadores.
- **Eventos en vivo** (SSE) con hora, accion, proceso, pid, uid y destino.
- Editar la **politica** (modo + blocklist) y guardarla en caliente.
- **Pausar / reanudar** el enforcement sin reiniciar.
- **Parar** el agente de forma limpia.

Si defines `token`, el panel pide el token y la API admite `Authorization: Bearer <token>` (el SSE tambien acepta `?token=`).

## API HTTP

| Metodo | Ruta | Descripcion |
|---|---|---|
| GET | /api/status | Estado, contadores, blocklist y version |
| GET | /api/events | Stream SSE de eventos en vivo |
| GET | /api/events/recent | Ultimos eventos en memoria |
| GET | /api/policy | Politica actual |
| PUT | /api/policy | Reemplaza la politica |
| POST | /api/block | Anade un CIDR: `{"cidr":"10.0.0.0/8"}` |
| DELETE | /api/block | Elimina un CIDR |
| POST | /api/enforce | Pausa/reanuda: `{"enforce":false}` |
| POST | /api/shutdown | Detiene el agente |

Ejemplos:

~~~bash
curl -s localhost:8787/api/status
curl -s -X POST -H 'Content-Type: application/json' -d '{"cidr":"1.1.1.1"}' localhost:8787/api/block
curl -s -X POST -H 'Content-Type: application/json' -d '{"enforce":false}' localhost:8787/api/enforce
~~~

## Configuracion

`config/agent.yaml`:

~~~yaml
listen: "127.0.0.1:8787"     # panel (usa 0.0.0.0:8787 + token para exponerlo)
token: ""                    # token en claro (vacio = sin auth)
token_file: "/etc/soar-agent/token"  # alternativa recomendada: fichero 0600
panel: true                  # false = agente sin interfaz (solo enforcement)
cgroup: "/sys/fs/cgroup"     # raiz = todo el sistema
policy: "/etc/soar-agent/policy.yaml"
log: "/var/log/soar-agent/events.jsonl"
history: 500
~~~

`config/policy.yaml`:

~~~yaml
mode: enforce                # enforce | monitor
blocklist:
  - 203.0.113.10
  - 198.51.100.0/24
~~~

## Despliegue como servicio

~~~bash
sudo deploy/install.sh
systemctl status soar-agent
journalctl -u soar-agent -f
~~~

Esto compila, instala el binario en /usr/local/bin, la config en /etc/soar-agent, genera un token y habilita el arranque automatico. Con `--no-enable` instala y arranca, pero sin autostart.

Gestion diaria:

~~~bash
bash scripts/panel.sh on|off|status   # encender/apagar el panel (sin parar el enforcement)
bash scripts/token.sh show|generate|clear
~~~

## Seguridad

- Enganchar a /sys/fs/cgroup afecta a **todo el sistema**; usa `cgroup` para acotarlo.
- El **loopback nunca se bloquea** desde la API (auto-proteccion del panel).
- El panel escucha en **127.0.0.1** por defecto: no es accesible desde la red.
- Define un **token** (`token` o `token_file`): cualquier proceso local puede alcanzar el panel.
- Para exponerlo en red: **token + TLS** (o mejor, un tunel SSH: `ssh -L 8787:127.0.0.1:8787 host`).
- Puedes **apagar el panel** sin parar el enforcement: `scripts/panel.sh off`.
- Un bug en el enforcement puede cortar trafico legitimo: prueba en **monitor** primero.

## Como funciona (notas tecnicas)

- La blocklist vive en un **LPM trie** con la clave en **bytes de red** (asi el kernel compara prefijos correctamente).
- El modo monitor usa un **Array de control** en el mapa eBPF: pausar no desengancha el hook.
- Los eventos viajan por **ring buffer** y se reparten con un canal **broadcast** a los clientes SSE.
- Toda mutacion de mapas pasa por un **unico actor**, evitando compartir el Ebpf entre tareas.

## Roadmap

- [x] **Fase 1** - Observacion y bloqueo de conexiones salientes (eBPF).
- [x] **Fase 2** - Servicio autonomo, politica YAML y panel web.
- [ ] **Fase 3** - Publicacion de eventos a un bus (NATS) y motor SOAR de playbooks.
- [ ] **Fase 4** - Mas acciones: matar proceso, cuarentena con fanotify, contencion de usuario.
- [ ] IPv6 (connect6) y UDP (sendmsg).

## Desarrollo

~~~bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
cargo clippy --release
~~~

Este repositorio es **autodescriptivo para agentes**: mira AGENTS.md y .agents/ (notas tecnicas y skills).

## Licencia

MIT. El programa eBPF se declara Dual MIT/GPL para poder usar helpers GPL-only.
