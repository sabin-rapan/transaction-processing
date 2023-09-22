pub mod handler;
pub mod server;
pub mod state;

use tokio::sync::mpsc::Receiver;

/// Run the engine.
pub async fn run(rx: Receiver<server::Command>) {
    let mut listener = server::Listener::new(rx);

    listener.run().await
}
