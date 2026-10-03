use std::io::Write;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = crumb_cli::Cli::parse();
    let outcome = crumb_cli::run(cli, &mut std::io::stdin()).await;
    let _ = std::io::stdout().write_all(outcome.stdout.as_bytes());
    let _ = std::io::stderr().write_all(outcome.stderr.as_bytes());
    std::process::exit(outcome.code);
}
