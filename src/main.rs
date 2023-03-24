use tracing::error;
use tracing::metadata::LevelFilter;
use tracing_subscriber::prelude::*;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match try_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("Exiting with an error...\n{err:?}");
            ExitCode::FAILURE
        }
    }
}

async fn try_main() -> anyhow::Result<()> {
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true).compact();

    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("TSTICK_LOG")
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        .init();

    tstick::run().await?;

    Ok(())
}
