//! API HTTP y panel web embebido.

use std::{convert::Infallible, sync::Arc, sync::atomic::Ordering};

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::{
    policy::{Mode, Policy},
    state::{AppState, Command, EventOut},
};

pub async fn serve(state: Arc<AppState>) -> anyhow::Result<()> {
    let addr: std::net::SocketAddr = state.config.listen.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("Panel web escuchando en http://{addr}");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

fn router(state: Arc<AppState>) -> Router {
    // La API va protegida con token; el dashboard (/) es publico para que pueda cargar y
    // pedir el token. Si / tambien exigiera token, el panel nunca llegaria a abrirse.
    let api = Router::new()
        .route("/api/status", get(status))
        .route("/api/events", get(events))
        .route("/api/events/recent", get(recent))
        .route("/api/policy", get(get_policy).put(put_policy))
        .route("/api/block", post(add_block).delete(remove_block))
        .route("/api/enforce", post(set_enforce))
        .route("/api/shutdown", post(shutdown))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth));

    Router::new().route("/", get(index)).merge(api).with_state(state)
}

/// Exige token si la configuracion define uno. Acepta cabecera Bearer o ?token= (para SSE).
async fn auth(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(token) = state.token.as_deref() else {
        return Ok(next.run(req).await);
    };
    let header_ok = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == token)
        .unwrap_or(false);
    let query_ok = req
        .uri()
        .query()
        .and_then(|q| {
            q.split('&').find_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                (k == "token").then_some(v)
            })
        })
        .map(|t| t == token)
        .unwrap_or(false);
    if header_ok || query_ok {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let (mode, blocklist) = {
        let p = state.policy.lock().unwrap();
        (p.mode, p.blocklist.clone())
    };
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "enforcing": state.enforcing.load(Ordering::Relaxed),
        "mode": match mode { Mode::Enforce => "enforce", Mode::Monitor => "monitor" },
        "uptime_s": state.started.elapsed().as_secs(),
        "allowed": state.allowed.load(Ordering::Relaxed),
        "blocked": state.blocked.load(Ordering::Relaxed),
        "blocklist": blocklist,
        "listen": state.config.listen,
        "auth": state.token.is_some(),
    }))
}

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|r| r.ok())
        .map(|ev| {
            Ok(Event::default()
                .event("connect")
                .data(serde_json::to_string(&ev).unwrap_or_default()))
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn recent(State(state): State<Arc<AppState>>) -> Json<Vec<EventOut>> {
    Json(state.history.lock().unwrap().iter().cloned().collect())
}

async fn get_policy(State(state): State<Arc<AppState>>) -> Json<Policy> {
    Json(state.policy.lock().unwrap().clone())
}

async fn put_policy(State(state): State<Arc<AppState>>, Json(policy): Json<Policy>) -> StatusCode {
    let _ = state.cmd_tx.send(Command::ReplacePolicy(policy)).await;
    StatusCode::ACCEPTED
}

#[derive(Deserialize)]
struct CidrBody {
    cidr: String,
}

async fn add_block(State(state): State<Arc<AppState>>, Json(body): Json<CidrBody>) -> StatusCode {
    let _ = state.cmd_tx.send(Command::AddBlock(body.cidr)).await;
    StatusCode::ACCEPTED
}

async fn remove_block(State(state): State<Arc<AppState>>, Json(body): Json<CidrBody>) -> StatusCode {
    let _ = state.cmd_tx.send(Command::RemoveBlock(body.cidr)).await;
    StatusCode::ACCEPTED
}

#[derive(Deserialize)]
struct EnforceBody {
    enforce: bool,
}

async fn set_enforce(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EnforceBody>,
) -> StatusCode {
    let _ = state.cmd_tx.send(Command::SetEnforce(body.enforce)).await;
    StatusCode::ACCEPTED
}

async fn shutdown(State(state): State<Arc<AppState>>) -> StatusCode {
    let _ = state.cmd_tx.send(Command::Shutdown).await;
    StatusCode::ACCEPTED
}
