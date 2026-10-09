use crate::domain::params::{hash_is_valid, PowParams};
use crate::mine::backend::Backend;
use crate::mine::worker::{self, FoundShare};
use crate::net::client;
use crate::net::egress::Proxy;
use crate::net::session::WsSession;
use crate::support::consts::{
    BASE_URL, CLOSE_GRACE_SECONDS, PROGRESS_SECONDS, SESSION_STATE_PATH, SHARE_PAUSE_MS,
    WS_CONNECT_ATTEMPTS, WS_CONNECT_RETRY_SECONDS, WS_RECONNECT_ATTEMPTS, WS_RECONNECT_WAIT_SECONDS,
};
use crate::support::error::{QmsError, QmsResult};
use crate::support::log::{lg, live_end_for, live_for, ly, wait_seconds};
use crate::support::shutdown;
use base64::Engine;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct MineOutcome {
    pub shares: u64,
    pub accepted: u64,
    pub hashrate: f64,
    pub balance: f64,
}

pub async fn start_session(
    http: &reqwest::Client,
    address: &str,
    captcha_token: &str,
) -> QmsResult<Value> {
    let url = format!("{}/api/startSession?cliver=2.5.1", BASE_URL);
    let body = json!({ "addr": address, "captchaToken": captcha_token });
    let rsp = client::post_json(http, &url, &body).await?;

    if let Some(code) = rsp.get("failedCode").and_then(|v| v.as_str()) {
        if !code.is_empty() {
            return Err(QmsError::Session(code.to_string()));
        }
    }

    Ok(rsp)
}

pub fn preimage_of(session: &Value) -> QmsResult<Vec<u8>> {
    let encoded = session
        .get("modules")
        .and_then(|m| m.get("pow"))
        .and_then(|p| p.get("preImage"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| QmsError::Session(String::from("faucet did not send a preimage")))?;

    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| QmsError::Session(String::from("faucet preimage was not valid base64")))
}

pub fn last_nonce_of(session: &Value) -> u64 {
    session
        .get("modules")
        .and_then(|m| m.get("pow"))
        .and_then(|p| p.get("lastNonce"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
}

pub fn read_state() -> Value {
    match std::fs::read_to_string(SESSION_STATE_PATH) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    }
}

fn write_state(state: &Value) {
    let _ = std::fs::write(SESSION_STATE_PATH, state.to_string());
}

pub fn store_session(address: &str, session_id: &str) {
    let mut state = read_state();
    if let Some(obj) = state.as_object_mut() {
        obj.insert(address.to_string(), Value::String(session_id.to_string()));
    }
    write_state(&state);
}

pub fn clear_session(address: &str) {
    let mut state = read_state();
    if let Some(obj) = state.as_object_mut() {
        obj.remove(address);
    }
    write_state(&state);
}

pub async fn release_stale(address: &str, proxy: Option<&Proxy>) {
    if let Some(id) = state_id(address) {
        let label = short_address(address);
        report(address, &label, close_session(&id, proxy).await, true);
    }
}

pub async fn release_blocked(address: &str, proxy: Option<&Proxy>) {
    let label = short_address(address);
    let mut found = false;
    let mut released = false;

    if let Some(id) = state_id(address) {
        found = true;
        if close_session(&id, proxy).await {
            released = true;
        }
    }

    if !released {
        if let Some(id) = live_session_for(address, proxy).await {
            found = true;
            if close_session(&id, proxy).await {
                released = true;
            }
        }
    }

    report(address, &label, released, found);
}

fn report(address: &str, label: &str, released: bool, found: bool) {
    if released {
        clear_session(address);
        lg(&format!(
            "Leftover session for the wallet {} was released",
            label
        ));
    } else if found && mark_warned(address) {
        ly(&format!(
            "Leftover session for the wallet {} could not be released",
            label
        ));
    }
}

fn mark_warned(address: &str) -> bool {
    static WARNED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    let set = WARNED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
    match set.lock() {
        Ok(mut s) => s.insert(address.to_string()),
        Err(_) => false,
    }
}

fn state_id(address: &str) -> Option<String> {
    read_state()
        .get(address)
        .and_then(|v| v.as_str())
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
}

async fn close_session(session_id: &str, proxy: Option<&Proxy>) -> bool {
    match WsSession::connect(session_id, proxy).await {
        Ok(mut ws) => {
            let _ = ws.send_close(1).await;
            let _ = tokio::time::timeout(
                Duration::from_secs(CLOSE_GRACE_SECONDS),
                ws.next_message(),
            )
            .await;
            true
        }
        Err(_) => false,
    }
}

async fn live_session_for(address: &str, proxy: Option<&Proxy>) -> Option<String> {
    let http = client::build(proxy).ok()?;
    let url = format!("{}/api/getFaucetStatus", BASE_URL);
    let status = client::get_json(&http, &url).await.ok()?;
    let sessions = status.get("sessions")?.as_array()?;
    for session in sessions {
        let target = session.get("target").and_then(|v| v.as_str()).unwrap_or("");
        if !target.eq_ignore_ascii_case(address) {
            continue;
        }
        let state = session.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if state == "running" {
            if let Some(id) = session.get("id").and_then(|v| v.as_str()) {
                return Some(id.to_string());
            }
        }
    }
    None
}

async fn abandon(session_id: &str, proxy: Option<&Proxy>) {
    let _ = close_session(session_id, proxy).await;
}

pub async fn mine(
    address: &str,
    session: &Value,
    params: &PowParams,
    backend: &Backend,
    max_shares: u64,
    target: f64,
    proxy: Option<&Proxy>,
) -> QmsResult<MineOutcome> {
    let session_id = session
        .get("session")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return Err(QmsError::Session(String::from(
            "faucet did not return a session id",
        )));
    }

    store_session(address, &session_id);

    let preimage = match preimage_of(session) {
        Ok(p) => p,
        Err(e) => {
            abandon(&session_id, proxy).await;
            clear_session(address);
            return Err(e);
        }
    };
    let label = short_address(address);

    let mut ws = match connect_ws(&session_id, proxy).await {
        Ok(ws) => ws,
        Err(e) => {
            abandon(&session_id, proxy).await;
            clear_session(address);
            return Err(e);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    let hashes = Arc::new(AtomicU64::new(0));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<FoundShare>(64);
    let first_nonce = last_nonce_of(session).saturating_add(1);
    let miner = {
        let params = params.clone();
        let backend = backend.clone();
        let preimage = preimage.clone();
        let stop = stop.clone();
        let hashes = hashes.clone();
        tokio::task::spawn_blocking(move || {
            worker::run(
                &params,
                &preimage,
                &backend,
                first_nonce,
                stop,
                hashes,
                tx,
            )
        })
    };

    let started = Instant::now();
    let mut state = MineState {
        nonce: last_nonce_of(session),
        sent: 0,
        accepted: 0,
        request_id: 1,
        balance: 0.0,
    };
    live_for(Some(&label), &status_line(&state, &hashes, started, &label));

    let mut reconnects: u32 = 0;
    let outcome: QmsResult<()> = loop {
        match pump(
            &mut ws,
            &mut state,
            &mut rx,
            &hashes,
            started,
            params,
            &preimage,
            &label,
            max_shares,
            target,
        )
        .await
        {
            Ok(Step::Finished) => break Ok(()),
            Ok(Step::Dropped(e)) => {
                if shutdown::requested() {
                    break Ok(());
                }
                if reconnects >= WS_RECONNECT_ATTEMPTS {
                    break Err(e);
                }
                reconnects += 1;
                live_end_for(Some(&label));
                ly(&format!(
                    "Socket dropped so the wallet {} reconnects",
                    label
                ));
                wait_seconds(WS_RECONNECT_WAIT_SECONDS).await;
                if shutdown::requested() {
                    break Ok(());
                }
                match connect_ws(&session_id, proxy).await {
                    Ok(next) => ws = next,
                    Err(fresh) => break Err(fresh),
                }
            }
            Err(e) => break Err(e),
        }
    };

    let interrupted = shutdown::requested();
    live_end_for(Some(&label));

    if interrupted {
        ly(&format!(
            "Stop was requested so the wallet {} closes its session",
            label
        ));
    }

    stop.store(true, Ordering::Relaxed);
    let closed = ws.send_close(state.request_id).await;
    if closed.is_ok() {
        let _ = tokio::time::timeout(
            Duration::from_secs(CLOSE_GRACE_SECONDS),
            ws.next_message(),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        clear_session(address);
    } else {
        store_session(address, &session_id);
    }
    drop(rx);
    let _ = miner.await;

    if interrupted {
        if closed.is_ok() {
            lg(&format!(
                "Faucet session for the wallet {} was closed cleanly",
                label
            ));
        } else {
            ly(&format!(
                "Faucet session for the wallet {} stayed open",
                label
            ));
        }
    }

    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    outcome?;

    Ok(MineOutcome {
        shares: state.sent,
        accepted: state.accepted,
        hashrate: hashes.load(Ordering::Relaxed) as f64 / elapsed,
        balance: state.balance,
    })
}

struct MineState {
    nonce: u64,
    sent: u64,
    accepted: u64,
    request_id: u64,
    balance: f64,
}

enum Step {
    Finished,
    Dropped(QmsError),
}

async fn connect_ws(session_id: &str, proxy: Option<&Proxy>) -> QmsResult<WsSession> {
    let mut last: Option<QmsError> = None;
    for attempt in 1..=WS_CONNECT_ATTEMPTS {
        match WsSession::connect(session_id, proxy).await {
            Ok(ws) => return Ok(ws),
            Err(e) => {
                last = Some(e);
                if attempt == WS_CONNECT_ATTEMPTS || shutdown::requested() {
                    break;
                }
                ly("Socket connect failed so another attempt is prepared");
                wait_seconds(WS_CONNECT_RETRY_SECONDS).await;
            }
        }
    }
    Err(last.unwrap_or_else(|| {
        QmsError::Network(String::from("socket connect failed for every attempt"))
    }))
}

fn status_line(state: &MineState, hashes: &AtomicU64, started: Instant, label: &str) -> String {
    let rate =
        hashes.load(Ordering::Relaxed) as f64 / started.elapsed().as_secs_f64().max(1.0);
    format!(
        "{} Mining {} H/s, {} sent, {} accepted, balance {:.2} {}",
        label,
        rate as u64,
        state.sent,
        state.accepted,
        state.balance,
        crate::support::log::coin_symbol()
    )
}

fn payout_reached(state: &MineState, target: f64, label: &str) -> bool {
    if target > 0.0 && state.balance >= target {
        live_end_for(Some(label));
        lg(&format!("Wallet {} reached the target of {:.2}", label, target));
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
async fn pump(
    ws: &mut WsSession,
    state: &mut MineState,
    rx: &mut tokio::sync::mpsc::Receiver<FoundShare>,
    hashes: &AtomicU64,
    started: Instant,
    params: &PowParams,
    preimage: &[u8],
    label: &str,
    max_shares: u64,
    target: f64,
) -> QmsResult<Step> {
    let params_str = params.params_str();
    let mut progress = tokio::time::interval(Duration::from_secs(PROGRESS_SECONDS));
    progress.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    progress.tick().await;

    loop {
        tokio::select! {
            _ = shutdown::wait() => {
                live_end_for(Some(label));
                return Ok(Step::Finished);
            }
            _ = progress.tick() => {
                live_for(Some(label), &status_line(state, hashes, started, label));
            }
            found = rx.recv() => {
                let share = match found {
                    Some(s) => s,
                    None => return Ok(Step::Finished),
                };
                if share.nonce <= state.nonce {
                    continue;
                }
                state.nonce = share.nonce;
                state.sent += 1;
                let rate = hashes.load(Ordering::Relaxed) as f64
                    / started.elapsed().as_secs_f64().max(1.0);
                if let Err(e) = ws
                    .send_share(
                        state.request_id,
                        share.nonce,
                        &share.hash_hex,
                        &params_str,
                        rate,
                    )
                    .await
                {
                    return Ok(Step::Dropped(e));
                }
                state.request_id += 1;
                if payout_reached(state, target, label) {
                    return Ok(Step::Finished);
                }
                if state.sent >= max_shares {
                    return Ok(Step::Finished);
                }
                tokio::time::sleep(Duration::from_millis(SHARE_PAUSE_MS)).await;
            }
            message = ws.next_message() => {
                let value = match message {
                    Ok(Some(v)) => v,
                    Ok(None) => return Ok(Step::Finished),
                    Err(e) => return Ok(Step::Dropped(e)),
                };
                let action = value.get("action").and_then(|v| v.as_str()).unwrap_or_default();
                match action {
                    "ok" => {
                        state.accepted += 1;
                        live_for(Some(label), &status_line(state, hashes, started, label));
                    }
                    "verify" => {
                        if let Err(e) = answer_verify(ws, &value, params, preimage, label).await {
                            return Ok(Step::Dropped(e));
                        }
                    }
                    "updateBalance" => {
                        state.balance = value
                            .get("data")
                            .and_then(|d| d.get("balance"))
                            .and_then(|b| b.as_str())
                            .and_then(|x| x.parse::<u128>().ok())
                            .unwrap_or(0) as f64
                            / 1e18;
                        live_for(Some(label), &status_line(state, hashes, started, label));
                        if payout_reached(state, target, label) {
                            return Ok(Step::Finished);
                        }
                    }
                    "error" => {
                        let code = value
                            .get("data")
                            .and_then(|d| d.get("code"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("UNKNOWN");
                        live_end_for(Some(label));
                        ly(&format!(
                            "Faucet rejected a share for the wallet {} with {}",
                            label, code
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn answer_verify(
    ws: &mut WsSession,
    value: &Value,
    params: &PowParams,
    fallback: &[u8],
    label: &str,
) -> QmsResult<()> {
    let data = match value.get("data") {
        Some(d) => d,
        None => return Ok(()),
    };

    let share_id = data
        .get("shareId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let nonce = data.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0);
    let claimed = data
        .get("data")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let preimage = match data.get("preimage").and_then(|v| v.as_str()) {
        Some(encoded) => base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap_or_else(|_| fallback.to_vec()),
        None => fallback.to_vec(),
    };

    let mask = params.difficulty_mask();
    let valid = match worker::hash_nonce(params, &preimage, nonce) {
        Some(hash) => hash_is_valid(&hash, &mask),
        None => false,
    };
    live_end_for(Some(label));
    if !valid {
        ly(&format!("Verification request was answered as {}", valid));
    }
    let _ = claimed;

    ws.send_verify_result(&share_id, &params.params_str(), valid)
        .await
}

pub fn short_address(address: &str) -> String {
    if address.len() < 12 {
        return address.to_string();
    }
    format!("{}...{}", &address[..8], &address[address.len() - 4..])
}
