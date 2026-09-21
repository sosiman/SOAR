# AGENTS.md - soar-agent

Guia para agentes que trabajen en este repositorio. Leer antes de modificar codigo.
Documentacion ampliada en .agents/notes/ . Skill reutilizable en .agents/skills/soar-ebpf-agent/SKILL.md .

## Que es este proyecto

Agente SOAR (fase 1) en Rust + eBPF (Aya). Observa y BLOQUEA conexiones TCP salientes IPv4
directamente en el kernel, en la ruta de la syscall connect().

Puntos que no hay que malinterpretar:

- El enforcement se hace con **eBPF** enganchado a cgroup/connect4, NO con un modulo de kernel y
  NO accediendo al silicio. El kernel ya es la capa que habla con el hardware.
- Un SOAR completo es orquestacion (fases 3-4). Este repo es solo el agente de ejecucion local.
- Cubre IPv4 en connect(). IPv6 y otras acciones estan en el roadmap.

## Estructura

    soar-agent/                  workspace cargo (edition 2024)
      Cargo.toml                 deps del workspace (aya 0.14, aya-ebpf 0.2.1, aya-build 0.2.0)
      soar-agent-ebpf/           PROGRAMA eBPF (se compila con nightly + bpf-linker)
        src/main.rs              hook cgroup/connect4, LPM trie BLOCKLIST, RingBuf EVENTS
        src/lib.rs               solo para dar lib target (evita warning de build-dependency)
        build.rs                 exige bpf-linker en PATH
      soar-agent-common/         tipos compartidos (no_std; aya::Pod con feature user)
        src/lib.rs               ConnectEvent
      soar-agent/                BINARIO de usuario (stable)
        src/main.rs              CLI, blocklist, attach, consumo async del ring buffer
        build.rs                 invoca aya_build::build_ebpf(Toolchain::Nightly)
      scripts/                   setup.sh, build.sh, run.sh
      .agents/                   notas tecnicas + skills del proyecto
      AGENTS.md                  este archivo

## Toolchain (ya instalado y verificado en esta maquina)

- Kernel 7.0.0-31-generic, cgroup v2, BTF en /sys/kernel/btf/vmlinux.
- Rust stable 1.98.1 y nightly 1.100.0 con el componente rust-src (obligatorio para -Z build-std).
- clang/LLVM 18.1.3.
- bpf-linker 0.11.1 en ~/.cargo/bin (binario precompilado musl; ver troubleshooting).
- sudo sin contrasena (se necesita root para cargar eBPF).

Instalacion reproducible: scripts/setup.sh

## Comandos

    export PATH="$HOME/.cargo/bin:$PATH"   # cargo/bpf-linker no estan en PATH por defecto

    scripts/build.sh                        # o: cargo build --release
    sudo ./target/release/soar-agent --log-allowed
    sudo ./target/release/soar-agent --block 1.1.1.1 --block 10.0.0.0/8
    scripts/run.sh --block 203.0.113.5

    # Prueba de bloqueo (el fallo debe ser inmediato, ~0 ms):
    sudo ./target/release/soar-agent --block 1.1.1.1 &
    curl -m 5 http://1.1.1.1        # exit 7, "Couldn't connect" en 0 ms
    curl -m 5 http://example.com    # HTTP 200
    kill %1

    # Debug del kernel:
    sudo bpftool prog show | grep -i cgroup
    sudo bpftool map show | grep -Ei 'BLOCKLIST|EVENTS'

## Invariantes tecnicos (no romper)

1. Valores de retorno del hook: 1 = permitir, 0 = denegar (EPERM). Constantes
   sk_action::SK_PASS / SK_DROP. NO invertirlos.
2. Orden de bytes del LPM trie: la clave lleva los bytes de red tal cual.
   - eBPF: sock_addr.user_ip4 ya es esa clave; usarla directa.
   - Usuarios: (u32::from(ip) & mask_por_prefijo).to_be(). NUNCA from_be_bytes para la clave.
   - Para el evento (orden de host): u32::from_be_bytes(ip_key.to_ne_bytes()).
   - Sintoma de error: IPs con bytes intercambiados y CIDR que no matchea.
3. Puerto: bpf_sock_addr.user_port es u32 con el puerto en orden de red en los 16 bits bajos:
   u16::from_be((user_port & 0xffff) as u16).
4. ConnectEvent es repr(C) sin padding (32 bytes). Cualquier campo nuevo debe quedar identico en
   kernel y usuarios: el tipo vive en soar-agent-common y ambos lados lo comparten.
5. reserve_bytes, no reserve<T>: en aya-ebpf 0.2.1 generic_const_exprs esta desactivado
   (rust-lang#141492), asi que RingBuf::reserve<T>() no existe. Usar
   EVENTS.reserve_bytes(core::mem::size_of::<T>(), 0).
6. Nombre del objeto eBPF: el bin del crate ebpf se llama soar-agent y aya-build lo copia a
   OUT_DIR; el usuarios lo incrusta con include_bytes_aligned!(concat!(env!("OUT_DIR"), "/soar-agent")).
   Si se renombra, cambiar ambos sitios.
7. Attach type: lo deduce Aya del nombre de seccion del ELF (cgroup/connect4); no se pasa a mano.
8. Rust 2024: usar #[unsafe(link_section = "...")] y #[unsafe(no_mangle)].
9. Sin panic en eBPF: hay #[panic_handler] que hace loop; no usar unwrap/panic en ese crate.

## Flujo de trabajo

1. Si tocas soar-agent-common, reconstruye todo: afecta a kernel y usuarios (layout del evento).
2. scripts/build.sh debe terminar SIN warnings. Hoy compila limpio.
3. Prueba siempre el binario real con root y comprueba el desenganche: tras matar el agente, la IP
   bloqueada debe volver a conectar (el bpf_link se libera al morir el proceso).
4. No commitear target/ (ya esta en .gitignore).
5. Si descubres algo nuevo (otro error de verificador, otra ruta de DSH), anotalo en .agents/notes/.

## Seguridad

- Enganchar a /sys/fs/cgroup afecta a TODO el sistema. Usa --cgroup para limitar el alcance.
- Nunca meter en la blocklist la IP de gestion del propio host.
- El agente debe salir limpiamente con Ctrl-C; el bpf_link se libera solo.

## Troubleshooting rapido

Detalle en .agents/notes/troubleshooting.md. Los tres mas probables:

- "library -lLLVM not found" al instalar bpf-linker: usar el prebuilt musl, no cargo install.
- "method reserve not found": usar reserve_bytes.
- El programa no carga con EPERM: no estas corriendo como root.

## Roadmap

Detalle en .agents/notes/roadmap.md.
