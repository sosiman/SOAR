use anyhow::{Context as _, anyhow};
use aya_build::Toolchain;

/// Compila el crate soar-agent-ebpf para el target BPF con nightly y lo deja en OUT_DIR,
/// de donde include_bytes_aligned! lo incrusta en el binario de usuario.
fn main() -> anyhow::Result<()> {
    let cargo_metadata::Metadata { packages, .. } = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .context("MetadataCommand::exec")?;
    let ebpf_package = packages
        .into_iter()
        .find(|cargo_metadata::Package { name, .. }| name.as_str() == "soar-agent-ebpf")
        .ok_or_else(|| anyhow!("paquete soar-agent-ebpf no encontrado"))?;
    let cargo_metadata::Package {
        name,
        manifest_path,
        ..
    } = ebpf_package;
    let ebpf_package = aya_build::Package {
        name: name.as_str(),
        root_dir: manifest_path
            .parent()
            .ok_or_else(|| anyhow!("sin directorio padre para {manifest_path}"))?
            .as_str(),
        ..Default::default()
    };
    aya_build::build_ebpf([ebpf_package], Toolchain::default())
}
