use crate::ops::{bootstrap, pipeline};
use crate::support::cfg::Config;
use crate::support::error::QmsResult;
use crate::support::log::lg;

pub async fn execute(cfg: &Config, account_path: &str, proxy_path: &str) -> QmsResult<()> {
    let boot = bootstrap::load(account_path, proxy_path).await?;
    lg("Starting automation cycle number 1");
    let report = pipeline::run_accounts(&boot, cfg).await?;
    lg("Automation cycle number 1 is complete");
    lg(&format!(
        "Account totals {:.2} {} and {:.2} {} earned",
        report.balance, boot.coin, report.earned, boot.coin
    ));
    bootstrap::closing_report(&boot).await;
    Ok(())
}
