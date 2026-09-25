//! Loads a page through the headless Chromium fallback: `cargo run --example browser_fetch -- <url>`
#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).expect("usage: browser_fetch <url>");
    let browser = crumb::browser::Browser::from_env();
    match browser.fetch(&url).await {
        Ok(html) => {
            let recipe = crumb::scraper::parse_recipe_html(&html, &url);
            println!(
                "{} bytes; recipe: {:?}",
                html.len(),
                recipe.map(|r| r.title)
            );
            let title = html
                .split("<title>")
                .nth(1)
                .and_then(|t| t.split('<').next())
                .unwrap_or("");
            println!("title: {title}");
        }
        Err(err) => println!("error: {err}"),
    }
}
