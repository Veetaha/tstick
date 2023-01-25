use tracing::error;
use tracing::metadata::LevelFilter;
use tracing_subscriber::prelude::*;

fn main() {
    if let Err(err) = try_main() {
        error!("Exitting with an error...\n{err:?}");
    }
}

fn try_main() -> anyhow::Result<()> {
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true).compact();

    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .with_env_var("TSTICK_LOG")
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        .init();

    tstick::run()?;

    Ok(())
}
