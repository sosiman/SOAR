# eBPF internals

## Hook cgroup/connect4

Se declara con el macro de Aya:

    #[cgroup_sock_addr(connect4)]
    pub fn connect4(ctx: SockAddrContext) -> i32 { ... }

El macro genera una funcion #[no_mangle] con link_section "cgroup/connect4". Aya deduce el attach
type de esa seccion; el binario de usuario no lo pasa a mano.

Contexto: SockAddrContext tiene el campo publico sock_addr: *mut bpf_sock_addr. Se accede con
unsafe { &*ctx.sock_addr } y se leen user_ip4 y user_port.

## Valores de retorno

| Retorno | Constante | Efecto |
|---|---|---|
| 1 | sk_action::SK_PASS | connect() continua |
| 0 | sk_action::SK_DROP | connect() falla con EPERM |

En la prueba real, el curl bloqueado termino en 0 ms con exit 7 ("Couldn't connect"): no es un
timeout, es un rechazo inmediato del kernel.

## Orden de bytes (el error mas facil de cometer)

El kernel compara los bytes de la clave del LPM trie de izquierda a derecha (MSB primero). Por eso
la clave debe contener los OCTETOS DE RED tal cual.

- bpf_sock_addr.user_ip4 es __be32: al leerlo como u32 nativo ya tenemos los bytes de red como
  entero; se usa directo como data de la clave.
- En usuarios, el patron equivalente es (u32::from(ip) & mask).to_be().
- Para el evento NO se usa eso: se convierte a orden de host con
  u32::from_be_bytes(ip_key.to_ne_bytes()) y en usuarios se imprime con Ipv4Addr::from(host_u32).
- NUNCA usar u32::from_be_bytes(octetos) como data de la clave: rompe el matching CIDR.
- Sintoma de bug: 1.1.1.1 se ve bien (simetrica) pero 10.0.0.1 aparece como 1.0.0.10, y un /24 no
  bloquea. Probar siempre con una IP no simetrica (10.0.0.1, 104.20.23.154).

Puerto: user_port es un u32 con el puerto en orden de red en los 16 bits bajos:

    let port = u16::from_be((sock_addr.user_port & 0xffff) as u16);

## LPM trie

Declaracion en eBPF:

    #[map]
    static BLOCKLIST: LpmTrie<u32, u8> = LpmTrie::with_max_entries(4096, 0);

Clave en eBPF:

    use aya_ebpf::maps::lpm_trie::Key;
    BLOCKLIST.get(&Key::new(32, ip_key))   // consulta siempre con /32; el trie devuelve el prefijo
                                           // mas largo que haga match

Clave en usuarios (insert):

    use aya::maps::lpm_trie::{Key, LpmTrie};
    let mut trie: LpmTrie<_, u32, u8> = LpmTrie::try_from(ebpf.map_mut("BLOCKLIST")...)?;
    trie.insert(&Key::new(prefix_len, ip_be), &1u8, 0)?;

Para anadir un CIDR hay que enmascarar la IP por el prefijo antes de insertar:

    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    let ip_be = (u32::from(ip) & mask).to_be();

## Ring buffer y reserve_bytes

IMPORTANTE: en aya-ebpf 0.2.1, la cfg generic_const_exprs esta DESACTIVADA (el build.rs de
aya-ebpf la comenta por rust-lang#141492). Por eso RingBuf::reserve<T>() no existe y hay que usar
reserve_bytes con un tamano constante de compilacion:

    let size = core::mem::size_of::<ConnectEvent>();
    if let Some(mut slot) = EVENTS.reserve_bytes(size, 0) {
        unsafe { core::ptr::write_unaligned(slot.as_mut_ptr().cast::<ConnectEvent>(), event); }
        slot.submit(0);
    }

En usuarios:

    let map = ebpf.take_map("EVENTS")?;
    let ring = RingBuf::try_from(map)?;
    let mut ring = AsyncFd::new(ring)?;
    loop {
        tokio::select! {
            ready = ring.readable_mut() => {
                let mut guard = ready?;
                while let Some(item) = guard.get_inner_mut().next() {
                    // comprobar item.len() >= size_of::<ConnectEvent>() antes de leer
                }
                guard.clear_ready();   // imprescindible o se produce un busy-loop
            }
            _ = signal::ctrl_c() => break,
        }
    }

## Helpers usados (todos disponibles en cgroup/connect4; verificado al cargar)

- bpf_get_current_pid_tgid()  -> pid = valor >> 32
- bpf_get_current_uid_gid()   -> uid = valor & 0xffffffff
- bpf_get_current_comm()      -> [u8; 16] (TASK_COMM_LEN)
- helpers de ring buffer (reserve/submit)

## Reserva de memoria

bpf_get_current_comm devuelve Result<[u8;16], i32>; en no_std se usa unwrap_or([0u8;16]).

## eBPF no_std/no_main

El crate eBPF necesita:

    #![no_std]
    #![no_main]
    #[panic_handler] fn panic(...) -> ! { loop {} }
    #[unsafe(link_section = "license")] #[unsafe(no_mangle)] static LICENSE = b"Dual MIT/GPL\0";

La licencia Dual MIT/GPL permite usar helpers GPL-only.

## Rust 2024

Los atributos de enlace requieren la forma nueva: #[unsafe(link_section = "...")] y
#[unsafe(no_mangle)], no las formas antiguas.
