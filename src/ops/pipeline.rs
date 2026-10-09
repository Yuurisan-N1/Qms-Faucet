use crate::domain::account::Account;
use crate::mine::backend::Backend;
use crate::mine::hw;
use crate::net::client;
use crate::net::egress::Proxy;
use crate::ops::{bootstrap::Bootstrap, captcha, claim, mining};
use crate::support::cfg::Config;
use crate::support::consts::{MAX_SHARES, SESSION_ATTEMPTS, SESSION_RETRY_SECONDS};
use crate::support::error::{QmsError, QmsResult};
use crate::support::log::{countdown, lg, ly, short, unit_word};
use crate::support::shutdown;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct CycleReport {
    pub balance: f64,
    pub earned: f64,
    pub shares: u64,
}

pub async fn run_accounts(boot: &Bootstrap, cfg: &Config) -> QmsResult<CycleReport> {
    let accel = hw::probe_verified(&boot.params);
    let lanes = lanes_of(cfg, boot.accounts.len(), accel.is_some());
    let groups = group_accounts(boot, lanes);
    let threads = cpu_threads(lanes);
    let backend = Arc::new(match accel {
        Some(gpu) => Backend::Gpu(gpu),
        None => Backend::Cpu(crate::mine::cpu::CpuMiner::new(threads)),
    });
    crate::support::log::set_coin(&boot.coin);
    let target = target_of(cfg, boot.min_claim);
    crate::support::log::set_backend(&format!("{} {}", backend.label(), backend.short_name()));
    lg(&format!(
        "Mining {}, target {:.2} {}",
        describe(&backend, lanes, threads),
        target,
        boot.coin
    ));
    if backend.label() == "cpu" {
        ly(hw::diagnose());
    } else {
        lg(&format!("The accelerator is {}", backend.detail()));
    }
    if boot.egress.duplicated() {
        ly("proxy.txt repeats an entry so two lanes share one ip");
    }

    if boot.egress.len() > 0 && lanes > boot.egress.len() {
        ly(&format!(
            "{} lanes share {} proxies so some wallets use one ip",
            lanes,
            boot.egress.len()
        ));
    }

    let serial = lanes == 1;
    let starts = boot.start.clone();

    let permits = Arc::new(Semaphore::new(lanes.max(1)));
    let mut handles = Vec::new();

    for group in groups {
        let params = boot.params.clone();
        let coin = boot.coin.clone();
        let backend = backend.clone();
        let permits = permits.clone();
        let starts = starts.clone();

        handles.push(tokio::spawn(async move {
            let _permit = match permits.acquire().await {
                Ok(p) => p,
                Err(_) => return Vec::new(),
            };
            let mut done = Vec::new();
            for (position, (account, proxy)) in group.into_iter().enumerate() {
                if shutdown::requested() {
                    break;
                }
                let start = starts.get(account.index).copied().unwrap_or(0.0);
                open_block(serial && position > 0, proxy.as_ref(), &account, start, &coin);
                done.push(
                    process_account(
                        &account,
                        &params,
                        &coin,
                        &backend,
                        target,
                        proxy.as_ref(),
                    )
                    .await,
                );
            }
            done
        }));
    }

    let mut balance = 0.0;
    let mut earned = 0.0;
    let mut shares = 0u64;

    for handle in handles {
        match handle.await {
            Ok(rows) => {
                for (account, amount, count) in rows {
                    balance += amount;
                    earned += amount;
                    shares += count;
                    if amount > 0.0 {
                        lg(&format!(
                            "Wallet {} finished with {} coins from {} shares",
                            mining::short_address(&account.address),
                            amount,
                            count
                        ));
                    }
                }
            }
            Err(e) => ly(&format!("An account task could not be joined because {}", e)),
        }
    }

    Ok(CycleReport {
        balance,
        earned,
        shares,
    })
}

fn describe(backend: &Backend, lanes: usize, threads: usize) -> String {
    let t = format!("{} {}", threads, unit_word(threads as i64, "thread", "threads"));
    let w = format!("{} {}", lanes, unit_word(lanes as i64, "wallet", "wallets"));
    match backend.label() {
        "gpu" => format!("gpu {} with {} at a time", backend.short_name(), w),
        _ if lanes > 1 => format!("cpu with {}, {} at a time", t, w),
        _ => format!("cpu with {}, one wallet at a time", t),
    }
}

fn target_of(cfg: &Config, min_claim: f64) -> f64 {
    let want = cfg.target_qms();
    if want <= 0.0 {
        return min_claim;
    }
    if want < min_claim {
        ly(&format!(
            "The target {:.2} is under the faucet minimum so {:.2} is used",
            want, min_claim
        ));
        return min_claim;
    }
    want
}

fn group_accounts(boot: &Bootstrap, lanes: usize) -> Vec<Vec<(Account, Option<Proxy>)>> {
    let lanes = lanes.max(1);
    let mut groups: Vec<Vec<(Account, Option<Proxy>)>> = (0..lanes).map(|_| Vec::new()).collect();
    for account in boot.accounts.iter().cloned() {
        let lane = account.index % lanes;
        let proxy = boot.egress.pick(account.index).cloned();
        groups[lane].push((account, proxy));
    }
    groups.retain(|g| !g.is_empty());
    groups
}

fn lanes_of(cfg: &Config, accounts: usize, accel: bool) -> usize {
    let accounts = accounts.max(1);
    let want = cfg.workers();
    if want > 0 {
        return want.min(accounts);
    }
    if accel {
        accounts
    } else {
        1
    }
}

pub fn open_block(
    gap: bool,
    proxy: Option<&Proxy>,
    account: &Account,
    start: f64,
    coin: &str,
) {
    if gap {
        crate::support::log::gap();
    }
    if let Some(p) = proxy {
        ly(&format!("Using proxy {}", p.masked()));
    }
    lg(&format!(
        "Wallet {} starts at {:.2} {}",
        mining::short_address(&account.address),
        start,
        coin
    ));
}

fn cpu_threads(lanes: usize) -> usize {
    let base = crate::mine::cpu::resolve_threads(0);
    (base / lanes.max(1)).max(1)
}

async fn process_account(
    account: &Account,
    params: &crate::domain::params::PowParams,
    coin: &str,
    backend: &Backend,
    target: f64,
    proxy: Option<&crate::net::egress::Proxy>,
) -> (Account, f64, u64) {
    let label = mining::short_address(&account.address);
    let idle = (account.clone(), 0.0, 0u64);

    if shutdown::requested() {
        return idle;
    }

    let http = match client::build(proxy) {
        Ok(c) => c,
        Err(e) => {
            ly(&format!(
                "Wallet {} skipped with the code {}",
                label,
                short(&e.code(), 18)
            ));
            return idle;
        }
    };

    mining::release_stale(&account.address, proxy).await;

    let token = match captcha::solve().await {
        Ok(t) => t,
        Err(e) => {
            if shutdown::requested() {
                return idle;
            }
            ly(&format!(
                "Wallet {} skipped with the code {}",
                label,
                short(&e.code(), 18)
            ));
            return idle;
        }
    };

    let mut opened: Option<serde_json::Value> = None;
    let mut last: Option<QmsError> = None;
    for attempt in 1..=SESSION_ATTEMPTS {
        match mining::start_session(&http, &account.address, &token).await {
            Ok(s) => {
                opened = Some(s);
                break;
            }
            Err(e) => {
                let network = matches!(e, QmsError::Network(_));
                let busy = e.to_string().contains("CONCURRENCY_LIMIT");
                last = Some(e);
                if (!network && !busy) || attempt == SESSION_ATTEMPTS || shutdown::requested() {
                    break;
                }
                ly(&format!(
                    "Wallet {} retries the session request",
                    label
                ));
                if busy {
                    mining::release_blocked(&account.address, proxy).await;
                }
                countdown(SESSION_RETRY_SECONDS, "Another attempt starts in").await;
            }
        }
    }

    let session = match opened {
        Some(s) => s,
        None => {
            if shutdown::requested() {
                return idle;
            }
            ly(&format!(
                "Wallet {} skipped with the code {}",
                label,
                short(
                    &last
                        .map(|e| e.code())
                        .unwrap_or_else(|| String::from("session not opened")),
                    18
                )
            ));
            return idle;
        }
    };

    let session_id = session
        .get("session")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let outcome = match mining::mine(
        &account.address,
        &session,
        params,
        backend,
        MAX_SHARES,
        target,
        proxy,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => {
            if shutdown::requested() {
                return idle;
            }
            ly(&format!(
                "Wallet {} stopped mining with the code {}",
                label,
                short(&e.code(), 18)
            ));
            return idle;
        }
    };

    lg(&format!(
        "Wallet {} mined {} shares, {} accepted, {:.2} {}",
        label,
        outcome.shares,
        outcome.accepted,
        outcome.balance,
        crate::support::log::coin_symbol()
    ));

    let amount = if outcome.accepted == 0 {
        0.0
    } else {
        match claim::claim(&http, &session_id, outcome.balance, target).await {
            Ok(a) => a,
            Err(e) => {
                ly(&format!(
                    "Wallet {} could not claim with the code {}",
                    label,
                    short(&e.code(), 18)
                ));
                0.0
            }
        }
    };

    let _ = coin;
    (account.clone(), amount, outcome.accepted)
}
