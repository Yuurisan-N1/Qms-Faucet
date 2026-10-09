use crate::mine::backend::Backend;
use crate::mine::cpu::CpuMiner;
use crate::mine::hw;
use crate::ops::{bootstrap, mining};
use crate::net::client;
use crate::support::cfg::Config;
use crate::support::error::QmsResult;
use crate::support::log::{lg, ly};

pub async fn execute(cfg: &Config, account_path: &str, proxy_path: &str) -> QmsResult<()> {
    let boot = bootstrap::load(account_path, proxy_path).await?;
    let threads = crate::mine::cpu::resolve_threads(0);
    let backend = match hw::probe_verified(&boot.params) {
        Some(gpu) => Backend::Gpu(gpu),
        None => Backend::Cpu(CpuMiner::new(threads)),
    };
    let target = if cfg.target_qms() > boot.min_claim {
        cfg.target_qms()
    } else {
        boot.min_claim
    };
    lg(&format!(
        "Mining backend is {}, target {:.2} {}",
        backend.label(),
        target,
        boot.coin
    ));
    if backend.label() == "cpu" {
        ly(hw::diagnose());
    }

    for (position, account) in boot.accounts.iter().enumerate() {
        let label = mining::short_address(&account.address);
        let proxy = boot.egress.pick(account.index).cloned();
        let start = boot.start.get(account.index).copied().unwrap_or(0.0);
        crate::ops::pipeline::open_block(
            position > 0,
            proxy.as_ref(),
            account,
            start,
            &boot.coin,
        );
        let http = client::build(proxy.as_ref())?;
        let token = match crate::ops::captcha::solve().await {
            Ok(t) => t,
            Err(e) => {
                ly(&format!("Wallet {} skipped with the code {}", label, e.code()));
                continue;
            }
        };
        let session = match mining::start_session(&http, &account.address, &token).await {
            Ok(s) => s,
            Err(e) => {
                ly(&format!("Wallet {} skipped with the code {}", label, e.code()));
                continue;
            }
        };
        let outcome = mining::mine(
            &account.address,
            &session,
            &boot.params,
            &backend,
            crate::support::consts::MAX_SHARES,
            target,
            proxy.as_ref(),
        )
        .await?;
        lg(&format!(
            "Wallet {} mined {} shares at {} H/s",
            label, outcome.accepted, outcome.hashrate as u64
        ));
    }
    Ok(())
}
