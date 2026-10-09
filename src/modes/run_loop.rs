use crate::ops::{bootstrap, pipeline};
use crate::support::cfg::Config;
use crate::support::error::QmsResult;
use crate::support::log::{close_live, countdown, lg, ly, short};
use crate::support::shutdown;

pub async fn execute(cfg: &Config, account_path: &str, proxy_path: &str) -> QmsResult<()> {
    let boot = bootstrap::load(account_path, proxy_path).await?;
    let mut cycle: u64 = 1;

    loop {
        lg(&format!("Starting automation cycle number {}", cycle));

        match pipeline::run_accounts(&boot, cfg).await {
            Ok(report) => {
                lg(&format!("Automation cycle number {} is complete", cycle));
                lg(&format!(
                    "Account totals {:.2} {} and {:.2} {} earned",
                    report.balance, boot.coin, report.earned, boot.coin
                ));
            }
            Err(e) => {
                ly(&format!("Automation cycle number {} ended early", cycle));
                ly(&format!("Reason is {}", short(&e.to_string(), 48)));
            }
        }

        if shutdown::requested() {
            lg("Stop was requested so the automation loop is ending");
            break;
        }

        bootstrap::closing_report(&boot).await;

        cycle += 1;
        countdown(cfg.sleep_seconds(), "Next cycle starts in").await;

        if shutdown::requested() {
            lg("Stop was requested so the automation loop is ending");
            break;
        }
    }

    close_live();
    Ok(())
}
