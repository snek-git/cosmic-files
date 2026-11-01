//! org.freedesktop.FileManager1, so "Show in folder" in browsers and editors opens here.
//! Each call starts a new window with the same binary: a folder opens as a tab, a file
//! opens its parent with the file selected.

use std::env;

struct FileManager;

async fn open(uris: Vec<String>, startup_id: String) -> zbus::fdo::Result<()> {
    if uris.is_empty() {
        return Ok(());
    }
    let exe = env::current_exe().map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
    let mut command = tokio::process::Command::new(exe);
    command.args(&uris);
    if !startup_id.is_empty() {
        command.env("XDG_ACTIVATION_TOKEN", startup_id);
    }
    // The child forks itself and its parent exits at once, so this wait is short.
    let status = command
        .status()
        .await
        .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
    if !status.success() {
        log::warn!("failed to open {uris:?}: {status}");
    }
    Ok(())
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    async fn show_folders(&self, uris: Vec<String>, startup_id: String) -> zbus::fdo::Result<()> {
        open(uris, startup_id).await
    }

    async fn show_items(&self, uris: Vec<String>, startup_id: String) -> zbus::fdo::Result<()> {
        open(uris, startup_id).await
    }

    async fn show_item_properties(
        &self,
        uris: Vec<String>,
        startup_id: String,
    ) -> zbus::fdo::Result<()> {
        open(uris, startup_id).await
    }
}

/// Owns the name on the session bus and serves calls until the process is stopped.
pub fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let _connection = zbus::connection::Builder::session()?
            .name("org.freedesktop.FileManager1")?
            .serve_at("/org/freedesktop/FileManager1", FileManager)?
            .build()
            .await?;
        std::future::pending::<()>().await;
        Ok(())
    })
}
