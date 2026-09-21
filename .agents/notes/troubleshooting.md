# Troubleshooting (errores reales de este proyecto)

## 1. cargo install bpf-linker falla con -lLLVM

Sintoma:

    rust-lld: error: unable to find library -lLLVM
    warning: bpf-linker@0.11.1: found LLVM library directory: /usr/lib64
    error: could not compile bpf-linker (bin "bpf-linker") due to 1 previous error

Causa: el build busca libLLVM en /usr/lib64 (solo el loader en Ubuntu noble) y no en
/usr/lib/llvm-18/lib; no hay libLLVM.so descubrible para -lLLVM.

Arreglo: no usar cargo install. Descargar el prebuilt musl (LLVM estatico) de GitHub releases:

    URL=https://github.com/aya-rs/bpf-linker/releases/download/v0.11.1/bpf-linker-x86_64-unknown-linux-musl.tar.zst
    curl -sSL "$URL" -o /tmp/bl.tar.zst
    zstd -d -f /tmp/bl.tar.zst -o /tmp/bl.tar
    tar xf /tmp/bl.tar -C ~/.cargo/bin && chmod +x ~/.cargo/bin/bpf-linker

Nota: los releases 0.11.x publican .tar.zst musl, NO .tar.gz gnu. Buscar el asset con "musl".

## 2. "bpf-linker no esta en el PATH" al compilar

Causa: soar-agent-ebpf/build.rs hace which("bpf-linker").expect(...) y ~/.cargo/bin no esta en PATH.

Arreglo:

    export PATH="$HOME/.cargo/bin:$PATH"

## 3. Error de build-std (nightly o rust-src ausente)

Causa: aya-build compila el eBPF con -Z build-std=core en nightly, que necesita el componente
rust-src. Si falta el toolchain nightly, ni siquiera arranca.

Arreglo:

    rustup toolchain install nightly --profile minimal --component rust-src

## 4. reserve<T> no existe (generic_const_exprs desactivado)

Sintoma: error de metodo no encontrado al usar EVENTS.reserve::<T>() .

Causa: aya-ebpf/build.rs comenta la cfg generic_const_exprs (rust-lang#141492), asi que esa API
queda deshabilitada.

Arreglo: usar reserve_bytes con un tamano constante:

    let size = core::mem::size_of::<ConnectEvent>();
    if let Some(mut slot) = EVENTS.reserve_bytes(size, 0) { ... }

## 5. warning: ignoring invalid dependency soar-agent-ebpf which is missing a lib target

Causa: el crate eBPF es solo binario y se declara como build-dependency del crate de usuario.

Arreglo: anadir soar-agent-ebpf/src/lib.rs con #![no_std] para darle un lib target.

## 6. El programa no carga: EPERM / Operation not permitted

Causa: cargar eBPF y enganchar cgroups requiere root (CAP_BPF / CAP_SYS_ADMIN).

Arreglo: ejecutar con sudo. El binario comprueba geteuid() y avisa si no es root.

## 7. CIDR que no bloquea o IPs con bytes intercambiados

Causa: clave del LPM trie construida en orden de host (from_be_bytes) en vez de orden de red.

Arreglo: clave = (u32::from(ip) & mask).to_be(); en eBPF usar user_ip4 directo. Probar con una IP
no simetrica (10.0.0.1, 104.20.23.154); 1.1.1.1 no revela el bug.

## 8. Busy-loop consumiendo CPU

Causa: no llamar a guard.clear_ready() tras drenar el ring buffer en el bucle de AsyncFd.

Arreglo: drenar con while let Some(item) = ...next() y despues clear_ready().

## 9. La skill no aparece en el catalogo

Causa: DSH descubre skills de proyecto en <projectRoot>/.agents/skills/<nombre>/SKILL.md y
<projectRoot>/.dsh/skills/<nombre>/SKILL.md, y projectRoot se marca con .git.

Arreglo: que exista .git en la raiz del proyecto y que el fichero se llame exactamente SKILL.md
dentro de un directorio con el nombre kebab-case. El catalogo se calcula por workspace/sesion: hay
que abrir el workspace en este proyecto (o reiniciar la sesion) para verla.

## 10. Los logs info! no salen

Causa: env_logger::init() por defecto solo muestra error.

Arreglo: env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init() .
Ya aplicado en soar-agent/src/main.rs.

## 11. El build muestra "warning: soar-agent@0.1.0: ..." por todas partes

Causa: es la salida del build.rs (aya-build compilando el eBPF), reemitida por cargo con ese
prefijo. No son warnings del crate de usuario.

Arreglo: ninguno; es normal. Buscar al final las lineas "Compiling soar-agent" y "Finished".

## 12. cargo no encontrado en una shell nueva

Causa: ~/.cargo/bin no esta en PATH.

Arreglo: export PATH="$HOME/.cargo/bin:$PATH" (o reabrir sesion si rustup edito el perfil).

## Diagnostico del kernel

    sudo bpftool prog show | grep -i cgroup      # programa cargado y enganchado
    sudo bpftool map show | grep -Ei 'BLOCKLIST|EVENTS'
    ps -ef | grep soar-agent                     # si hay un agente vivo
    # tras matar el agente, la IP bloqueada debe volver a conectar (link liberado)
