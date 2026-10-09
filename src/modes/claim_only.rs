use crate::net::client;
use crate::ops::{bootstrap, captcha, claim, mining};
use crate::support::cfg::Config;
use crate::support::error::QmsResult;
use crate::support::log::{lg, ly};

pub async fn execute(cfg: &Config, account_path: &str, proxy_path: &str) -> QmsResult<()> {
    let boot = bootstrap::load(account_path, proxy_path).await?;
    let _ = cfg;

    for account in boot.accounts.iter() {
        let label = mining::short_address(&account.address);
        let proxy = boot.egress.pick(account.index).cloned();
        let http = client::build(proxy.as_ref())?;

        let token = match captcha::solve().await {
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

        let session_id = session
            .get("session")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        match claim::claim(&http, &session_id, 0.0, boot.min_claim).await {
            Ok(amount) => lg(&format!("Wallet {} claimed {} coins", label, amount)),
            Err(e) => ly(&format!("Wallet {} could not claim because {}", label, e)),
        }
    }
    Ok(())
}
