mod api;
mod indexer;
mod porter;
mod query;
mod ranker;
mod semantic;
mod snippet;
mod trie;
mod typo;

use std::sync::Arc;
use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Router,
    response::Html,
};
use tokio::sync::mpsc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

use api::SharedState;
use indexer::Index;

const INDEX_PATH: &str = "search_index.bin";
const BIND_ADDR:  &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging ───────────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // ── Index bootstrap ───────────────────────────────────────────────────────
    let mut index = Index::load_or_new(INDEX_PATH);

    if index.total_docs == 0 {
        info!("Seeding sample corpus…");
        for (title, body, created_at) in indexer::sample_corpus() {
            index.add_document_at(title, body, created_at);
        }
        index.save(INDEX_PATH)?;
    }

    info!(
        docs  = index.total_docs,
        vocab = index.postings.len(),
        "Index ready"
    );

    // ── Background indexer channel ────────────────────────────────────────────
    // Channel capacity = 64: back-pressures callers if the worker falls behind.
    let (index_tx, index_rx) = mpsc::channel::<api::IndexTask>(64);

    // ── Shared state ──────────────────────────────────────────────────────────
    let state: SharedState = Arc::new(
        api::AppState::new(index, INDEX_PATH, index_tx)
    );

    // Spawn the background indexer task.
    tokio::spawn(api::background_indexer(Arc::clone(&state), index_rx));

    // ── CORS ──────────────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // ── Router ────────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/", get(|| async {
            Html(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/index.html")))
        }))
        .route("/api/search",  post(api::search_handler))
        .route("/api/index",   post(api::index_handler))
        .route("/api/suggest", get(api::suggest_handler))
        .route("/api/stats",   get(api::stats_handler))
        .route("/api/health",  get(api::health_handler))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // ── Serve ─────────────────────────────────────────────────────────────────
    let addr: SocketAddr = BIND_ADDR.parse()?;
    info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
