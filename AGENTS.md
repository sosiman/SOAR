# AGENTS.md - soar-agent

Guia para agentes que trabajen en este repositorio. Leer antes de modificar codigo.
Notas ampliadas en .agents/notes/ . Skill reutilizable en .agents/skills/soar-ebpf-agent/SKILL.md .

## Que es este proyecto

Agente SOAR autonomo en Rust + eBPF (Aya): observa y BLOQUEA conexiones TCP salientes IPv4
directamente en el kernel, y se controla desde un panel web. Pensado para publicarse y desplegarse
como servicio.

- Enforcement con **eBPF** (cgroup/connect4), NO con modulo de kernel ni "hablando al silicio".
- La orquestacion SOAR (playbooks) es la fase 3; esto es el agente de ejecucion + su panel.
- Cubre IPv4 en connect(). IPv6/UDP estan en el roadmap.

## Estructura

    soar-agent/                  workspace cargo (edition 2024)
      Cargo.toml                 workspace
      soar-agent-ebpf/           PROGRAMA eBPF (nightly + bpf-linker)
        src/main.rs              hook cgroup/connect4, BLOCKLIST (LPM), CONTROL (Array), EVENTS (RingBuf)
        src/lib.rs               lib target (evita warning de build-dependency)
      soar-agent-common/         ConnectEvent compartido (no_std; aya::Pod con feature user)
      soar-agent/                BINARIO de usuario (stable)
        src/main.rs              CLI, arranque, ensamblado de piezas
        src/config.rs            Config YAML
        src/policy.rs            Policy (enforce/monitor + blocklist CIDR) y parse_cidr
        src/state.rs             AppState + Command + EventOut
        src/actor.rs             actor que posee el eBPF (mapas + ring buffer + ordenes)
        src/api.rs               API HTTP + panel (axum) + auth por token
        static/index.html        dashboard embebido (SSE + controles)
      config/                    agent.yaml y policy.yaml de ejemplo
      deploy/                    soar-agent.service (systemd) e install.sh
      scripts/                   setup.sh, build.sh, run.sh
      .agents/                   notas tecnicas + skills
      README.md                  cara publica (GitHub/LinkedIn)
      AGENTS.md                  este archivo

## Toolchain (ya instalado y verificado)

- Kernel 7.0.0-31-generic, cgroup v2, BTF en /sys/kernel/btf/vmlinux.
- Rust stable 1.98.1 y nightly 1.100.0 con rust-src.
- clang/LLVM 18.1.3; bpf-linker 0.11.1 (prebuilt musl) en ~/.cargo/bin.
- sudo sin contrasena. Instalacion reproducible: scripts/setup.sh

## Comandos

    export PATH="$HOME/.cargo/bin:$PATH"

    scripts/build.sh                                   # o cargo build --release
    sudo ./target/release/soar-agent --config config/agent.yaml
    sudo ./target/release/soar-agent --listen 127.0.0.1:8787 --monitor
    sudo deploy/install.sh                             # servicio systemd autonomo

    # API / panel
    curl -s localhost:8787/api/status
    curl -s -X POST -H 'Content-Type: application/json' -d '{"cidr":"1.1.1.1"}' localhost:8787/api/block
    curl -s -X POST -H 'Content-Type: application/json' -d '{"enforce":false}' localhost:8787/api/enforce
    curl -s -X POST localhost:8787/api/shutdown

    # Debug kernel
    sudo bpftool prog show | grep -i cgroup
    sudo bpftool map show | grep -Ei 'BLOCKLIST|CONTROL|EVENTS'

## Invariantes tecnicos (no romper)

1. Retorno del hook: 1 = permitir (SK_PASS), 0 = denegar (SK_DROP -> EPERM). NO invertir.
2. Orden de bytes del LPM trie: clave en bytes de red.
   - eBPF: sock_addr.user_ip4 se usa directo.
   - Usuarios: (u32::from(ip) & mask).to_be(). NUNCA from_be_bytes para la clave.
   - Evento (orden de host): u32::from_be_bytes(ip_key.to_ne_bytes()).
3. Puerto: u16::from_be((user_port & 0xffff) as u16).
4. ConnectEvent repr(C) sin padding (32 bytes); vive en soar-agent-common y lo comparten ambos lados.
5. reserve_bytes, no reserve<T> (generic_const_exprs desactivado en aya-ebpf 0.2.1).
6. El objeto eBPF se llama soar-agent (bin del crate ebpf) y se incrusta desde OUT_DIR.
7. Attach type lo deduce Aya de la seccion ELF (cgroup/connect4).
8. Rust 2024: #[unsafe(link_section = "...")] y #[unsafe(no_mangle)].
9. ENABLE/DISABLE: el modo monitor escribe 0 en CONTROL[0]; el hook NO se desengancha.
10. Seguridad del panel: el loopback no se bloquea y las peticiones al puerto del panel no se
    registran. No quitar esas protecciones.
11. Tipos de mapa en usuarios: MapData, no el enum Map (try_from lo desempaqueta). Ej:
    LpmTrie<MapData, u32, u8>, Array<MapData, u32>, RingBuf<MapData>.
12. Toda mutacion de mapas pasa por el actor (un solo dueno del Ebpf). La API solo envia Command.

## Flujo de trabajo

1. Tocar soar-agent-common obliga a reconstruir kernel y usuarios (layout del evento).
2. scripts/build.sh debe terminar SIN warnings.
3. Prueba SIEMPRE el binario real (root) y el ciclo completo por API: status, block, enforce,
   shutdown. Verifica que tras parar, la IP vuelve a conectar (bpf_link liberado).
4. No commitear target/.
5. Nuevos hallazgos -> .agents/notes/ .

## Seguridad

- Enganchar a /sys/fs/cgroup afecta a TODO el sistema; usa --cgroup para acotar.
- Loopback protegido por diseno; no romper esa proteccion.
- Para exponer el panel en red: token + TLS delante.

## Troubleshooting rapido

Detalle en .agents/notes/troubleshooting.md. Los mas probables:

- "library -lLLVM not found" al instalar bpf-linker: usar el prebuilt musl.
- trait bound Map: Borrow<MapData> is not satisfied: usar MapData en los tipos.
- El programa no carga con EPERM: falta root.

## Roadmap

Detalle en .agents/notes/roadmap.md. Fase 1 y 2 HECHAS; fase 3 = SOAR con playbooks.
