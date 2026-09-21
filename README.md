# soar-agent

Agente de ejecucion de acciones **SOAR** a nivel de **kernel** sobre Linux, escrito en **Rust +
eBPF (Aya)**.

> **Fase 1 (MVP):** observa y **bloquea conexiones TCP salientes IPv4 en el kernel**, en la propia
> ruta de la syscall connect(), consultando una lista de bloqueo con soporte de CIDR.

## Por que eBPF y no un modulo del kernel

El hook cgroup/connect4 se ejecuta **dentro del kernel**, en el contexto de la syscall connect(),
antes de que se establezca la conexion. Devolver 0 hace que connect() falle con EPERM; devolver 1
la permite. Eso da control real de enforcement sin escribir un modulo:

- El **verificador** de eBPF garantiza que el programa no cuelga el kernel.
- Es **CO-RE**: no hay que recompilar por version de kernel.
- El programa no puede tocar memoria arbitraria ni llamar helpers fuera de la lista permitida.

## Arquitectura

        (kernel)  cgroup/connect4  -->  LPM trie BLOCKLIST   (decision: PASS / DROP)
             |
             |  RingBuf EVENTS (pid, uid, ip, puerto, comm, blocked)
             v
     (usuarios)  soar-agent  -->  consola / (roadmap) bus de eventos -> orquestador SOAR

- **soar-agent-ebpf**: programa eBPF. LpmTrie u32,u8 (CIDR) + RingBuf de eventos.
- **soar-agent-common**: ConnectEvent compartido (no_std; aya::Pod tras el feature user).
- **soar-agent**: binario de usuario (Aya + tokio). Rellena la blocklist, carga/engancha el
  programa y consume el ring buffer de forma asincrona.

## Requisitos

- Linux con **cgroup v2** y BTF en /sys/kernel/btf/vmlinux.
- Rust **stable** + **nightly** con el componente rust-src.
- clang/LLVM y **bpf-linker** (lo instala scripts/setup.sh).
- Privilegios de root para cargar programas eBPF.

Todo se instala con:

    scripts/setup.sh

## Compilar

    scripts/build.sh
    # o directamente
    cargo build --release

cargo build compila el crate eBPF con nightly (via aya-build) y lo incrusta en el binario de
usuario. El resultado es target/release/soar-agent.

## Uso

    # Observar todo (sin bloquear nada)
    sudo ./target/release/soar-agent --log-allowed

    # Bloquear una IP y una red completa
    sudo ./target/release/soar-agent --block 1.2.3.4 --block 10.0.0.0/8

    # O con el wrapper
    scripts/run.sh --block 203.0.113.5

Opciones:

| Opcion | Descripcion |
|---|---|
| --block CIDR | IP o CIDR a bloquear (repetible). |
| --cgroup PATH | Cgroup al que enganchar (por defecto /sys/fs/cgroup, todo el sistema). |
| --log-allowed | Registrar tambien las conexiones permitidas. |

### Prueba rapida

    # Terminal 1
    sudo ./target/release/soar-agent --block 1.1.1.1 --log-allowed

    # Terminal 2
    curl -m 5 http://1.1.1.1       # debe fallar (EPERM / permiso denegado)
    curl -m 5 http://9.9.9.9       # debe funcionar

La salida del agente muestra una linea por conexion, con BLOCK o ALLOW.

## Seguridad

- Enforce a nivel de kernel: una vez enganchado a la **cgroup raiz**, afecta a **todo el sistema**.
  Usa --cgroup para limitarlo a un subarbol de cgroup.
- **eBPF, no modulo**: el verificador evita que el agente tumbe la maquina.
- El bloqueo es reversible: al salir el proceso, el bpf_link se libera y el hook se desengancha.
- No metas en la blocklist la IP de gestion del propio host (te quedarias fuera).

## Roadmap

1. Fase 1 - Observacion + bloqueo de conexiones salientes (este repo).
2. Fase 2 - Politica declarativa (YAML firmado), allowlist, metricas y API local.
3. Fase 3 - Bus de eventos (NATS) y orquestador SOAR con playbooks.
4. Fase 4 - Mas acciones (matar proceso, cuarentena con fanotify) y conectores.

## Para agentes

Este repositorio es autodescriptivo para agentes:

- AGENTS.md - instrucciones del proyecto (se carga al abrir el workspace aqui).
- .agents/notes/ - memoria tecnica: arquitectura, toolchain, eBPF internals, troubleshooting, roadmap.
- .agents/skills/soar-ebpf-agent/SKILL.md - skill reutilizable con el procedimiento completo.

DSH descubre las skills de proyecto en .agents/skills/<nombre>/SKILL.md y .dsh/skills/<nombre>/SKILL.md,
y AGENTS.md en la raiz (marcada con .git).

## Licencia

MIT OR Apache-2.0. El programa eBPF se declara Dual MIT/GPL para poder usar helpers GPL-only.
