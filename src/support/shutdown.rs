use std::sync::OnceLock;
use tokio::sync::watch;

static CHANNEL: OnceLock<watch::Sender<bool>> = OnceLock::new();
static KEEP: OnceLock<watch::Receiver<bool>> = OnceLock::new();

pub fn init() {
    let (tx, rx) = watch::channel(false);
    let _ = CHANNEL.set(tx);
    let _ = KEEP.set(rx);
}

pub fn requested() -> bool {
    CHANNEL.get().map(|tx| *tx.borrow()).unwrap_or(false)
}

pub fn request() {
    if let Some(tx) = CHANNEL.get() {
        tx.send_replace(true);
    }
}

pub async fn wait() {
    let mut rx = match CHANNEL.get() {
        Some(tx) => tx.subscribe(),
        None => {
            std::future::pending::<()>().await;
            return;
        }
    };
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}
