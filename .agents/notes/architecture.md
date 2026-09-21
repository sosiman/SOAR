# Arquitectura

## Vision general

    (kernel)   cgroup/connect4  -->  LPM trie BLOCKLIST        decision: PASS / DROP
                    |
                    |  RingBuf EVENTS  (pid, uid, ip, puerto, comm, blocked)
                    v
    (usuarios) soar-agent  -->  stdout   [roadmap: NATS -> orquestador SOAR]

Dos procesos, no uno: el enforcement vive en el kernel (eBPF); la orquestacion SOAR es una capa
aparte en red. Acoplarlas en un monolito es un anti-patron: si cae el orquestador no debe caerse la
capacidad de enforcement, y viceversa.

## Crate por crate

| Crate | Target | Rol |
|---|---|---|
| soar-agent-ebpf | bpfel-unknown-none (nightly) | Hook cgroup/connect4, LPM trie, ring buffer |
| soar-agent-common | no_std (+feature user) | ConnectEvent compartido; aya::Pod en usuarios |
| soar-agent | x86_64 (stable) | CLI, poblado de mapas, attach, lectura async |

## Flujo de una conexion

1. Un proceso llama connect(fd, sockaddr) sobre IPv4.
2. El kernel invoca el programa eBPF enganchado a la cgroup, ANTES de establecer la conexion.
3. El programa lee bpf_sock_addr.user_ip4 y .user_port y consulta BLOCKLIST (longest prefix match).
4. Publica un ConnectEvent en el ring buffer EVENTS.
5. Devuelve 1 (SK_PASS) o 0 (SK_DROP). Con SK_DROP, connect() falla con EPERM.
6. El agente de usuario, despierto por AsyncFd, drena el ring buffer e imprime el evento.

## Mapas

| Mapa | Tipo | Tamano | Clave / valor |
|---|---|---|---|
| BLOCKLIST | LPM trie | 4096 | Key(prefix_len, ip en bytes de red) -> u8 = 1 |
| EVENTS | Ring buffer | 1 MiB | ConnectEvent (32 bytes) |

with_max_entries anade BPF_F_NO_PREALLOC automaticamente en LpmTrie (extra_flags del constructor).

## Layout de ConnectEvent (32 bytes, repr(C))

    offset  campo      tipo      nota
    0       pid        u32       pid_tgid >> 32 (tgid)
    4       uid        u32       uid_gid truncado a 32 bits
    8       dst_ip4    u32       orden de host
    12      dst_port   u16       orden de host
    14      blocked    u8        1 = bloqueada
    15      _pad       u8        alineacion explicita
    16      comm       [u8;16]   comm del hilo, con NUL de relleno

Cambiar este layout obliga a reconstruir kernel y usuarios a la vez. El tipo vive en
soar-agent-common para que no puedan divergir.

## Ciclo de vida del attach

- El blocklist se puebla con ebpf.map_mut("BLOCKLIST") dentro de un bloque cuyo prestamo termina
  antes de pedir el programa (el borrow checker no deja mantener ambos).
- program.load() valida con el verificador; program.attach(file, CgroupAttachMode::default())
  crea un bpf_link guardado dentro de los datos del programa en Ebpf.
- El link permanece mientras el proceso vive. Al salir (Ctrl-C o kill), el descriptor se cierra y
  el kernel desengancha el hook: la IP vuelve a conectar. Verificado en la prueba real.
- EVENTS se obtiene al final con ebpf.take_map("EVENTS") para no solapar prestamos.

## Decisiones y por que

- **eBPF y no modulo de kernel**: el verificador impide colgar la maquina; CO-RE evita recompilar
  por version; superficie de ataque mucho menor. Un modulo da mas poder pero un bug es un panic de
  kernel y hay que firmarlo.
- **cgroup/connect4 y no XDP/TC**: connect4 ve la intencion de conexion con identidad de proceso
  (pid/uid/comm) y puede denegarla con EPERM; XDP/TC trabaja a nivel de paquete, sin ese contexto.
- **Ring buffer y no perf events**: ordenado, sin perdidas por muestreo y con API de byte slice.
- **Clave en orden de red y evento en orden de host**: el kernel compara bytes crudos (necesita
  orden de red); la impresion es mas comoda en orden de host.
