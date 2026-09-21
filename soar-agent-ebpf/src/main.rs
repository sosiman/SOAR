#![no_std]
#![no_main]

//! Programa eBPF enganchado a cgroup/connect4.
//!
//! Se ejecuta DENTRO del kernel, en la ruta de la syscall connect() de IPv4. Devuelve
//! SK_DROP (0) para denegar con EPERM o SK_PASS (1) para permitir. La decision se toma
//! consultando un LPM trie (soporta CIDR) y cada intento se publica en un ring buffer.

use aya_ebpf::{
    bindings::sk_action,
    helpers::{bpf_get_current_comm, bpf_get_current_pid_tgid, bpf_get_current_uid_gid},
    macros::{cgroup_sock_addr, map},
    maps::{lpm_trie::Key, LpmTrie, RingBuf},
    programs::SockAddrContext,
};
use soar_agent_common::{ConnectEvent, TASK_COMM_LEN};

/// Lista de bloqueo: clave = (prefix_len, IPv4 en bytes de red), valor = 1.
#[map]
static BLOCKLIST: LpmTrie<u32, u8> = LpmTrie::with_max_entries(4096, 0);

/// Eventos hacia el agente de usuario (1 MiB, potencia de dos y multiplo de pagina).
#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(1 << 20, 0);

#[cgroup_sock_addr(connect4)]
pub fn connect4(ctx: SockAddrContext) -> i32 {
    match try_connect4(ctx) {
        Ok(action) => action,
        Err(action) => action,
    }
}

#[inline(always)]
fn try_connect4(ctx: SockAddrContext) -> Result<i32, i32> {
    // SAFETY: el kernel garantiza que el contexto es un bpf_sock_addr valido durante la llamada.
    let sock_addr = unsafe { &*ctx.sock_addr };

    // user_ip4 contiene los bytes de red como entero nativo; es exactamente la clave que espera
    // el LPM trie (lo mismo que u32::from(ip).to_be() en usuarios).
    let ip_key = sock_addr.user_ip4;

    // Para el evento si queremos orden de host, de cara a imprimirlo como a.b.c.d.
    let ip_host = u32::from_be_bytes(ip_key.to_ne_bytes());
    let port = u16::from_be((sock_addr.user_port & 0xffff) as u16);

    let blocked = BLOCKLIST.get(&Key::new(32, ip_key)).is_some();

    emit(ConnectEvent {
        pid: (bpf_get_current_pid_tgid() >> 32) as u32,
        uid: bpf_get_current_uid_gid() as u32,
        dst_ip4: ip_host,
        dst_port: port,
        blocked: blocked as u8,
        _pad: 0,
        comm: bpf_get_current_comm().unwrap_or([0u8; TASK_COMM_LEN]),
    });

    // 1 = permitir, 0 = denegar (EPERM). Son los valores del enum sk_action.
    Ok(if blocked {
        sk_action::SK_DROP as i32
    } else {
        sk_action::SK_PASS as i32
    })
}

#[inline(always)]
fn emit(event: ConnectEvent) {
    // size es una constante de compilacion: el verificador lo exige para reserve_bytes.
    let size = core::mem::size_of::<ConnectEvent>();
    if let Some(mut slot) = EVENTS.reserve_bytes(size, 0) {
        // SAFETY: slot tiene al menos size_of::<ConnectEvent>() bytes y esta alineado a 8.
        unsafe {
            core::ptr::write_unaligned(slot.as_mut_ptr().cast::<ConnectEvent>(), event);
        }
        slot.submit(0);
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
