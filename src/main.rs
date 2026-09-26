use axum::extract::Request;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use crumb::{AppState, app, browser::Browser, config::Config, db, telemetry};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

fn main() -> Result<(), BoxError> {
    // Sentry starts before the runtime so panics on any thread are reported; the guard flushes
    // queued events when main returns.
    let settings = telemetry::Settings::from_env();
    let sentry = telemetry::init(&settings);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(sentry.is_some().then(telemetry::tracing_layer))
        .init();
    tracing::info!("{}", telemetry::describe(&settings, sentry.is_some()));

    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve());
    drop(sentry);
    result
}

async fn serve() -> Result<(), BoxError> {
    let config = Config::from_env();
    let path = db::database_path();
    let database = db::open(&path)?;
    tracing::info!("database at {}", path.display());
    if !config.auth_enabled() {
        tracing::warn!(
            "APP_PASSWORD is not set: the app and connector are open to anyone who can reach them"
        );
    }

    let browser = Browser::from_env();
    tracing::info!(
        "browser fallback {}",
        if browser.available() {
            "enabled"
        } else {
            "unavailable"
        }
    );

    let addr = format!("{}:{}", config.host, config.port);
    let service = NormalizePathLayer::trim_trailing_slash()
        .layer(app(AppState::new(database, config, browser)));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Crumb listening on http://{addr}");
    // The peer address is the client's when no proxy is in front (share links' 404 limit)
    axum::serve(
        listener,
        axum::ServiceExt::<Request>::into_make_service_with_connect_info::<std::net::SocketAddr>(
            service,
        ),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
