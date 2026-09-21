#!/usr/bin/env bash
# Instala el toolchain necesario: Rust (stable + nightly con rust-src), clang/LLVM y bpf-linker.
set -euo pipefail

echo "==> Instalando dependencias del sistema (clang, LLVM, zstd)"
if command -v apt-get >/dev/null; then
  sudo apt-get update -y
  sudo apt-get install -y clang llvm lld libelf-dev libclang-dev pkg-config zstd curl
fi

echo "==> Instalando rustup + stable + nightly (con rust-src)"
if ! command -v rustup >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install nightly --profile minimal --component rust-src

echo "==> Instalando bpf-linker (binario precompilado musl)"
if ! command -v bpf-linker >/dev/null; then
  URL="https://github.com/aya-rs/bpf-linker/releases/download/v0.11.1/bpf-linker-x86_64-unknown-linux-musl.tar.zst"
  TMP="$(mktemp -d)"
  curl -sSL "$URL" -o "$TMP/bpf-linker.tar.zst"
  zstd -d -f "$TMP/bpf-linker.tar.zst" -o "$TMP/bpf-linker.tar"
  tar xf "$TMP/bpf-linker.tar" -C "$HOME/.cargo/bin"
  chmod +x "$HOME/.cargo/bin/bpf-linker"
  rm -rf "$TMP"
fi

echo "==> Versiones"
rustc --version
cargo --version
rustup toolchain list
bpf-linker --version
echo "Setup completado."
