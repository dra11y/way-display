use std::time::Duration;

use crate::Result;
use tokio::time::sleep;
use zbus::Connection;

/// Connect to the session bus
pub async fn connect(max_attempts: usize) -> Result<Connection> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        match Connection::session().await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                eprintln!("Failed to connect to session DBus (attempt {attempts}): {error}");
                if attempts < max_attempts {
                    sleep(Duration::from_secs(1)).await
                } else {
                    return Err(error.into());
                }
            }
        }
    }
}
