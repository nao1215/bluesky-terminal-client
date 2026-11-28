//! Asynchronous pictures, a bounded disk cache, and silent MP4/H.264 playback.
pub mod images;
mod mp4;
pub mod player;

fn spawn<T: Send + 'static>(
    name: &str,
    run: impl FnOnce() -> T + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<T>> {
    std::thread::Builder::new().name(name.into()).spawn(run)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .user_agent("terminal-kit")
        .build()
        .into()
}
