use std::process::ExitCode;

use crumb_relay::{Config, Relay, router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => {
            tracing::error!("{message}");
            return ExitCode::from(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(config.listen).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("can't listen on {}: {e}", config.listen);
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(
        "crumb-relay {} \"{}\" listening on {} (one fetch per site per {} s, {} at once, {} a minute)",
        env!("CARGO_PKG_VERSION"),
        config.name,
        config.listen,
        config.host_interval.as_secs(),
        config.concurrency,
        config.per_minute
    );
    if config.listen.ip().is_unspecified() {
        tracing::warn!(
            "listening on every interface; set RELAY_LISTEN to the Tailscale address (tailscale ip -4) so only your tailnet can reach it"
        );
    }

    let relay = Relay::new(&config);
    let can = relay.can();
    if can.is_empty() {
        tracing::info!("fetching only (no Chromium or video tools, or RELAY_WORK=off)");
    } else {
        tracing::info!(
            "also works for the server: {} ({} at once)",
            can.join(", "),
            config.heavy_workers
        );
    }
    let app = router(relay);
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await
    {
        tracing::error!("server stopped: {e}");
        return ExitCode::FAILURE;
    }
    tracing::info!("stopped");
    ExitCode::SUCCESS
}

/// Resolves on Ctrl-C or, under systemd and Docker, SIGTERM.
async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                term.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
    tracing::info!("shutting down");
}
