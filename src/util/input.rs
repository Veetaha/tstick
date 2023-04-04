use crate::prelude::*;
use anyhow::bail;

pub(crate) async fn read_confirmation(message: &str, auto_confirm: bool) -> Result {
    if auto_confirm {
        return Ok(());
    }

    warn!("{message} Only `yes` will be accepted to confirm");

    // Tokio recommends spawning a blocking thread for user input
    // https://docs.rs/tokio/latest/tokio/io/struct.Stdin.html
    let user_input = tokio::task::spawn_blocking(move || {
        std::io::stdin()
            .lines()
            .next()
            .transpose()
            .context("Failed to read confirmation from `stdin`")?
            .context("Reached end-of-file (EOF) while reading confirmation from `stdin`")
    })
    .await
    .expect("Failed to spawn blocking task for user input")?;

    let user_input = user_input.trim();

    if user_input != "yes" {
        bail!("Confirmation response was not `yes`");
    }

    Ok(())
}
