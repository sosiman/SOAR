use which::which;

/// El build depende de bpf-linker (no declarado en Cargo.toml). Recompilamos si cambia el
/// binario encontrado en el PATH.
fn main() {
    let bpf_linker = which("bpf-linker").expect("bpf-linker no esta en el PATH");
    println!("cargo:rerun-if-changed={}", bpf_linker.to_str().unwrap());
}
