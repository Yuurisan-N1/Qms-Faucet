use crate::support::error::{QmsError, QmsResult};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub id: String,
    pub status: String,
    pub balance: f64,
    pub target: String,
    pub start: i64,
}

impl SessionInfo {
    pub fn from_value(v: &Value) -> QmsResult<Self> {
        let id = v
            .get("session")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            return Err(QmsError::Session("faucet did not return a session id".to_string()));
        }
        Ok(Self {
            id,
            status: v
                .get("status")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .to_string(),
            balance: v
                .get("balance")
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            target: v
                .get("target")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string(),
            start: v.get("start").and_then(|x| x.as_i64()).unwrap_or(0),
        })
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
    }

    pub fn is_claimable(&self) -> bool {
        self.status == "claimable"
    }

    pub fn failed_code(&self, v: &Value) -> String {
        v.get("failedCode")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn failed_reason(&self, v: &Value) -> String {
        v.get("failedReason")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    }
}

pub fn drop_amount(v: &Value) -> f64 {
    v.get("balance")
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

pub fn wei_to_coin(wei: &str, decimals: u32) -> f64 {
    let value: f64 = wei.parse().unwrap_or(0.0);
    value / 10f64.powi(decimals as i32)
}

pub fn coin_symbol(cfg: &Value) -> String {
    cfg.get("faucetCoinSymbol")
        .and_then(|v| v.as_str())
        .unwrap_or("QMS")
        .to_string()
}

pub fn max_claim(cfg: &Value) -> f64 {
    cfg.get("maxClaim")
        .and_then(|v| v.as_u64())
        .map(|v| v as f64)
        .unwrap_or(0.0)
}

pub fn min_claim(cfg: &Value) -> f64 {
    let wei = cfg
        .get("minClaim")
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
        })
        .unwrap_or(0);
    if wei == 0 {
        1.0
    } else {
        wei as f64 / 1e18
    }
}

pub fn session_timeout(cfg: &Value) -> u64 {
    cfg.get("sessionTimeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(86400)
}
