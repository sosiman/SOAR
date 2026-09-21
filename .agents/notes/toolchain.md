# Toolchain

## Versiones verificadas en esta maquina

| Pieza | Version | Nota |
|---|---|---|
| Kernel | 7.0.0-31-generic | cgroup v2, BTF presente |
| Rust stable | 1.98.1 | compila el crate de usuario |
| Rust nightly | 1.100.0 (2026-09-21) | compila el eBPF; necesita rust-src |
| clang/LLVM | 18.1.3 | solo para depurar/objdump |
| bpf-linker | 0.11.1 | binario musl precompilado en ~/.cargo/bin |
| aya | 0.14.0 | libreria de usuarios |
| aya-ebpf | 0.2.1 | lado kernel |
| aya-build | 0.2.0 | orquesta el build del eBPF |
| edition | 2024 | workspace y crates |

## Instalacion reproducible

    scripts/setup.sh

Hace, en orden:

1. apt: clang, llvm, lld, libelf-dev, libclang-dev, pkg-config, zstd, curl.
2. rustup + stable, y nightly con el componente rust-src.
3. bpf-linker prebuilt musl (descarga de GitHub releases, se extrae en ~/.cargo/bin).

## Pipeline de build (lo que realmente ocurre con cargo build)

1. cargo (stable) compila soar-agent.
2. soar-agent/build.rs ejecuta aya_build::build_ebpf(..., Toolchain::default()).
   Toolchain::default() es Nightly, asi que internamente lanza rustup run nightly cargo ...
3. Esa compilacion usa -Z build-std=core y el target bpfel-unknown-none; por eso rust-src es
   obligatorio en el toolchain nightly.
4. bpf-linker enlaza el objeto eBPF.
5. aya-build copia el binario resultante a OUT_DIR/soar-agent.
6. include_bytes_aligned!(concat!(env!("OUT_DIR"), "/soar-agent")) lo incrusta en el binario de
   usuario. El .o no se carga de disco en runtime.

Consecuencia: un cambio en el crate eBPF se propaga al binario solo si el build.rs se re-ejecuta.
Si algo huele a objeto obsoleto, forzar con cargo clean o tocar el crate eBPF.

## Por que bpf-linker prebuilt y no cargo install

cargo install bpf-linker falla al enlazar en esta maquina:

    rust-lld: error: unable to find library -lLLVM

El build busca libLLVM en /usr/lib64 (que en Ubuntu noble solo contiene el loader) en lugar de en
/usr/lib/llvm-18/lib, y no hay libLLVM.so descubrible para -lLLVM. El binario precompilado musl
trae LLVM estatico y no depende del sistema.

URL usada (assets .tar.zst musl; los releases no publican .tar.gz gnu en 0.11.x):

    https://github.com/aya-rs/bpf-linker/releases/download/v0.11.1/bpf-linker-x86_64-unknown-linux-musl.tar.zst

Instalacion manual:

    curl -sSL <URL> -o /tmp/bl.tar.zst
    zstd -d -f /tmp/bl.tar.zst -o /tmp/bl.tar
    tar xf /tmp/bl.tar -C ~/.cargo/bin
    chmod +x ~/.cargo/bin/bpf-linker

Alternativa no probada: instalar llvm-18-dev y apuntar LLVM_SYS_180_PREFIX=/usr/lib/llvm-18.

## PATH

~/.cargo/bin no esta en el PATH por defecto en esta sesion. Cualquier build o script debe:

    export PATH="$HOME/.cargo/bin:$PATH"
