use std::io;

#[cfg(unix)]
pub(crate) async fn signal() -> io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result?;

            tracing::info!(signal = "SIGINT", "shutdown signal received");
        }

        received = terminate.recv() => {
            if received.is_none() {
                return Err(io::Error::other(
                    "SIGTERM listener closed unexpectedly"
                ));
            }

            tracing::info!(signal = "SIGTERM", "shutdown signal received");
        }
    }

    Ok(())
}

#[cfg(not(unix))]
pub(crate) async fn signal() -> io::Result<()> {
    tokio::signal::ctrl_c().await?;
    tracing::info!(signal = "SIGINT", "shutdown signal received");
    Ok(())
}
