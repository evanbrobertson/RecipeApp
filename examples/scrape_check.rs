//! Imports each URL through the scraper's fetch order and prints which method won:
//! `cargo run --release --example scrape_check -- <url>...`
//! Makes real requests; keep the list short.
use crumb::scraper::{Fetched, Method, fetch_wreq, scrape_with};
use std::time::Instant;

#[tokio::main]
async fn main() {
    let browser = std::sync::Arc::new(crumb::browser::Browser::from_env());
    for url in std::env::args().skip(1) {
        let started = Instant::now();
        let tried = std::cell::RefCell::new(Vec::new());
        let result = scrape_with(&url, browser.available(), |method| {
            let browser = browser.clone();
            let url = url.clone();
            let tried = &tried;
            async move {
                let at = Instant::now();
                let fetched = match method {
                    Method::Browser => match browser.fetch(&url).await {
                        Ok(html) => Fetched::Page { status: 200, html },
                        Err(err) => Fetched::Unreachable(err),
                    },
                    wreq => fetch_wreq(wreq, &url).await,
                };
                let what = match &fetched {
                    Fetched::Page { status, html } => format!("{status} ({} bytes)", html.len()),
                    Fetched::Unreachable(e) => format!("error: {e}"),
                };
                tried.borrow_mut().push(format!(
                    "{} {what} {}ms",
                    method.label(),
                    at.elapsed().as_millis()
                ));
                fetched
            }
        })
        .await;
        let host = crumb::telemetry::host_of(&url);
        let winner = match &result {
            Ok((method, crumb::scraper::Scraped { recipe, .. })) => format!(
                "{} \"{}\" prep={:?} cook={:?} extra={:?} total={:?}",
                method.label(),
                recipe.title,
                recipe.prep_time,
                recipe.cook_time,
                recipe.freeze_time,
                recipe.total_time
            ),
            Err(_) => "failed".into(),
        };
        println!(
            "{host}\t{winner}\t{}ms\t[{}]",
            started.elapsed().as_millis(),
            tried.borrow().join("; ")
        );
    }
}
