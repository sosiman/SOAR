# Contribuir

1. Abre un issue antes de cambios grandes.
2. Un cambio por pull request, con descripcion clara.
3. Ejecuta antes de enviar:

~~~bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt
cargo clippy --release
cargo build --release
~~~

4. Prueba el binario real (necesita root) y verifica el desbloqueo al salir.
5. Si tocas el eBPF, respeta los invariantes de `AGENTS.md` (orden de bytes, valores de retorno, layout del evento).

Al enviar un PR aceptas que tu aporte se publique bajo la licencia MIT del proyecto.
