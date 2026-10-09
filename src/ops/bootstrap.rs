use crate::domain::account::{self, Account};
use crate::domain::faucet;
use crate::domain::params::PowParams;
use crate::net::client;
use crate::net::egress::Egress;
use crate::net::rpc;
use crate::ops::mining;
use crate::support::consts::{BASE_URL, FALLBACK_CHAIN_ID};
use crate::support::error::QmsResult;
use crate::support::log::{lg, ly};
use serde_json::Value;

pub struct Bootstrap {
    pub config: Value,
    pub params: PowParams,
    pub coin: String,
    pub min_claim: f64,
    pub accounts: Vec<Account>,
    pub egress: Egress,
    pub start: Vec<f64>,
}

pub async fn load(account_path: &str, proxy_path: &str) -> QmsResult<Bootstrap> {
    let http = client::build(None)?;
    let url = format!("{}/api/getFaucetConfig?cliver=2.5.1", BASE_URL);
    let config = client::get_json(&http, &url).await?;
    let params = PowParams::from_config(&config)?;
    let coin = faucet::coin_symbol(&config);
    let min_claim = faucet::min_claim(&config);
    let accounts = account::load(account_path)?;
    let egress = Egress::load(proxy_path)?;

    if rpc::chain_id(&http).await != FALLBACK_CHAIN_ID {
        ly("The RPC chain does not match the faucet chain");
    }
    lg(&format!(
        "Faucet pays out from {:.2} {} per session",
        min_claim, coin
    ));
    if let Some(pool) = rpc::pool_balance(&http).await {
        lg(&format!("Faucet pool holds {:.2} {}", pool, coin));
    }

    let mut start = Vec::with_capacity(accounts.len());
    for account in accounts.iter() {
        let balance = rpc::balance_of(&http, &account.address).await.unwrap_or(0.0);
        start.push(balance);
    }

    Ok(Bootstrap {
        config,
        params,
        coin,
        min_claim,
        accounts,
        egress,
        start,
    })
}

pub async fn closing_report(boot: &Bootstrap) {
    let http = match client::build(None) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut gained = 0.0;
    for (index, account) in boot.accounts.iter().enumerate() {
        let now = rpc::balance_of(&http, &account.address).await.unwrap_or(0.0);
        let before = boot.start.get(index).copied().unwrap_or(0.0);
        let delta = now - before;
        gained += delta;
        if delta > 0.0 {
            lg(&format!(
                "Wallet {} now holds {:.2} {}",
                mining::short_address(&account.address),
                now,
                boot.coin
            ));
        }
    }
    if gained > 0.0 {
        lg(&format!(
            "Wallet balances gained {:.2} {} in total",
            gained, boot.coin
        ));
    }
}
