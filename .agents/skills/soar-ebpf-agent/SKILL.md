---
name: soar-ebpf-agent
description: Compilar, probar, extender y depurar el agente SOAR en Rust + eBPF (Aya) de /home/sosi/soar-agent. Usar cuando se trabaje en ese proyecto, se anada una accion de enforcement (bloqueo de IP, matar proceso, cuarentena), se toque el hook cgroup/connect4 o el LPM trie, o aparezcan errores del verificador eBPF, de bpf-linker o de aya-build.
---

# soar-ebpf-agent

Procedimiento operativo del proyecto /home/sosi/soar-agent . Antes de tocar nada, leer
/home/sosi/soar-agent/AGENTS.md . Notas ampliadas en .agents/notes/ .

## Contexto minimo

Agente SOAR fase 1 en Rust + eBPF (Aya). Enforcement real en el kernel via hook cgroup/connect4:
devuelve 1 (permitir) o 0 (denegar con EPERM). NO es un modulo de kernel ni toca el hardware.
Tres crates: soar-agent-ebpf (kernel), soar-agent-common (tipos), soar-agent (usuarios).

## Preparar el entorno

    export PATH="$HOME/.cargo/bin:$PATH"
    # si falta algo:
    scripts/setup.sh

## Compilar

    scripts/build.sh
    # equivalente:
    export PATH="$HOME/.cargo/bin:$PATH" && cargo build --release

Debe terminar sin warnings. El crate de usuario (stable) dispara aya-build, que compila el eBPF con
nightly (-Z build-std) y bpf-linker, y lo incrusta en el binario.

## Ejecutar y probar

    sudo ./target/release/soar-agent --log-allowed
    sudo ./target/release/soar-agent --block 1.1.1.1 --block 10.0.0.0/8

Prueba de aceptacion (el bloqueo debe ser inmediato, ~0 ms, no un timeout):

    sudo ./target/release/soar-agent --block 1.1.1.1 &
    curl -m 5 http://1.1.1.1        # exit 7, "Couldn't connect" en 0 ms
    curl -m 5 http://example.com    # HTTP 200
    kill %1
    sleep 1
    curl -m 5 http://1.1.1.1        # debe volver a conectar (link liberado)

## Anadir una accion de enforcement nueva

1. Nuevo hook en soar-agent-ebpf/src/main.rs (macro de Aya segun el tipo: cgroup_sock_addr,
   cgroup_sysctl, lsm, uprobe, etc.). Elegir el hook mas especifico que permita la decision.
2. Si hay estado nuevo, declarar un #[map] en eBPF y poblarlo desde usuarios con
   ebpf.map_mut(...) o ebpf.take_map(...) + try_from .
3. Si cambia el evento, editar ConnectEvent en soar-agent-common y reconstruir AMBOS lados.
4. En usuarios: program_mut(<nombre>) -> try_into() -> load() -> attach(...) .
5. Respetar los invariantes de AGENTS.md (retorno 1/0, orden de bytes, reserve_bytes, etc.).
6. Anadir la prueba al final de scripts/ o a estas instrucciones.

## Depurar

- Error del verificador: leer el log completo de carga; suele indicar helper no permitido, puntero
  no comprobado o desbordamiento de pila. Simplificar el programa hasta aislar.
- El programa no aparece: el attach falla o no eres root.
- Comprobar en el kernel:

      sudo bpftool prog show | grep -i cgroup
      sudo bpftool map show | grep -Ei 'BLOCKLIST|EVENTS'

- IP mal impresa o CIDR que no matchea: es un bug de orden de bytes (ver ebpf-internals.md).

## Errores conocidos (detalle en troubleshooting.md)

- cargo install bpf-linker -> "unable to find library -lLLVM": usar el prebuilt musl.
- "bpf-linker no esta en el PATH": exportar ~/.cargo/bin.
- "method reserve not found": usar reserve_bytes, no reserve<T>.
- EPERM al cargar: falta sudo.

## Reglas

- Enganchar a /sys/fs/cgroup afecta a todo el sistema; usar --cgroup para acotar.
- No meter en la blocklist la IP de gestion del propio host.
- Probar siempre el desbloqueo al salir del proceso.
- Actualizar .agents/notes/ cuando aparezca un error o un hallazgo nuevo.
