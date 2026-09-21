#![no_std]
//! Tipos compartidos entre el programa eBPF (cgroup/connect4) y el agente de usuario.
//!
//! Este crate se compila en dos contextos:
//! * eBPF (no_std, sin el feature user): define la estructura que viaja por el ring buffer.
//! * usuarios (feature user): anade el unsafe impl aya::Pod.

/// Longitud de comm en el kernel (TASK_COMM_LEN).
pub const TASK_COMM_LEN: usize = 16;

/// Evento emitido por el programa eBPF en cada intento de connect() IPv4.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConnectEvent {
    /// PID del hilo que realiza la conexion.
    pub pid: u32,
    /// UID del proceso.
    pub uid: u32,
    /// IP destino en orden de host.
    pub dst_ip4: u32,
    /// Puerto destino en orden de host.
    pub dst_port: u16,
    /// 1 si la conexion fue bloqueada; 0 si se permitio.
    pub blocked: u8,
    /// Relleno (alineacion explicita del layout repr(C)).
    pub _pad: u8,
    /// Nombre del proceso con NUL de relleno.
    pub comm: [u8; TASK_COMM_LEN],
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for ConnectEvent {}
