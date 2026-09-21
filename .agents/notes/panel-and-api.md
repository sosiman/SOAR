# Panel web y API

## Piezas

- `api.rs`: router axum con el panel (index.html embebido) y la API JSON/SSE.
- `actor.rs`: unico dueno del Ebpf. Recibe `Command` por canal mpsc y drena el ring buffer.
- `state.rs`: `AppState` compartido (canal de ordenes, broadcast de eventos, historial, contadores).
- `static/index.html`: dashboard (se incrusta con include_str!, no se sirve de disco).

## Por que un actor

`aya::Ebpf` no es `Sync` y los mapas necesitan `"mut`. En vez de compartirlo con candados, un solo lugar
(el actor) lo posee y recibe ordenes. El panel solo tiene `Arc<AppState>`, que si es `Send + Sync`.

## Flujo de una orden

1. El panel hace POST/PUT/DELETE a la API.
2. El handler envia un `Command` por `mpsc` y responde 202 Accepted (fire-and-forget).
3. El actor aplica el cambio al mapa eBPF y persiste la politica YAML.
4. El siguiente `GET /api/status` refleja el nuevo estado.

## Eventos en vivo

El actor publica cada evento en un canal `broadcast`. `GET /api/events` lo convierte en SSE
(`BroadcastStream` -> `Event`). El dashboard usa `EventSource` y pinta las filas al vuelo. El historial
reciente (`GET /api/events/recent`) rellena la tabla al abrir.

## Autenticacion

Si `config.token` no esta vacio, `auth` exige `Authorization: Bearer <token>` o el parametro
`?token=` (necesario para SSE, porque `EventSource` no permite cabeceras). El dashboard pide el token y lo
guarda en localStorage.

## Decisiones de seguridad

- `is_loopback_spec` rechaza 127.0.0.0/8 en la blocklist: evita que un admin se deje fuera del panel.
- `emit` descarta los eventos cuyo destino es el puerto del panel en loopback (ruido).
- El modo monitor usa `CONTROL[0] = 0` en vez de desenganchar: cambio instantaneo y reversible.

## API

| Metodo | Ruta | Cuerpo |
|---|---|---|
| GET | /api/status | - |
| GET | /api/events | - (SSE) |
| GET | /api/events/recent | - |
| GET | /api/policy | - |
| PUT | /api/policy | Policy (mode + blocklist) |
| POST | /api/block | `{"cidr":"..."}` |
| DELETE | /api/block | `{"cidr":"..."}` |
| POST | /api/enforce | `{"enforce":bool}` |
| POST | /api/shutdown | - |
