#![allow(dead_code)]

mod cuda;
mod domain;
mod mine;
mod modes;
mod net;
mod ops;
mod support;

use support::cfg::Config;
use support::error::QmsResult;
use crate::support::log::{lr, ly};

const CONFIG_PATH: &str = "config.json";
const ACCOUNT_PATH: &str = "address.txt";
const PROXY_PATH: &str = "proxy.txt";

#[tokio::main]
async fn main() {
    support::banner::print_banner();
    support::shutdown::init();

    tokio::spawn(async move {
        let mut hits: u32 = 0;
        while tokio::signal::ctrl_c().await.is_ok() {
            hits += 1;
            println!();
            if hits == 1 {
                ly("Stop was requested so the miners close their sessions");
                support::shutdown::request();
            } else {
                lr("Script stopped by user");
                std::process::exit(0);
            }
        }
    });

    #[cfg(unix)]
    tokio::spawn(async move {
        let mut term = match tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        term.recv().await;
        ly("Stop was requested so the miners close their sessions");
        support::shutdown::request();
    });

    if let Err(e) = dispatch().await {
        lr(&e.to_string());
        std::process::exit(1);
    }
    if support::shutdown::requested() {
        lr("Script stopped by user");
    }
}

async fn dispatch() -> QmsResult<()> {
    let cfg = Config::load(CONFIG_PATH)?;

    let mode = std::env::args().nth(1).unwrap_or_else(|| "loop".to_string());
    match mode.as_str() {
        "loop" => modes::run_loop::execute(&cfg, ACCOUNT_PATH, PROXY_PATH).await,
        "mine" => modes::mine_only::execute(&cfg, ACCOUNT_PATH, PROXY_PATH).await,
        "claim" => modes::claim_only::execute(&cfg, ACCOUNT_PATH, PROXY_PATH).await,
        "probe" => modes::probe::execute().await,
        "selftest" => modes::selftest::execute(&cfg).await,
        "once" => modes::run_once::execute(&cfg, ACCOUNT_PATH, PROXY_PATH).await,
        other => {
            ly(&format!("Unknown mode {} was requested so the loop is used", other));
            modes::run_loop::execute(&cfg, ACCOUNT_PATH, PROXY_PATH).await
        }
    }
}
