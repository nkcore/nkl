use clap::Parser;

mod certs;
mod cli;
mod config;
mod discover;
mod error;
mod hosts;
mod npx_guard;
mod pages;
mod proxy;
mod routes;
mod run;
mod status;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    npx_guard::check_npx_execution();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = cli::Cli::parse();
    cli.run().await
}
