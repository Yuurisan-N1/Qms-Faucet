use crate::net::client;
use crate::support::consts::{FALLBACK_CHAIN_ID, RPC_URL};
use serde_json::{json, Value};

async fn call(http: &reqwest::Client, method: &str, params: Value) -> Option<Value> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    match client::post_json(http, RPC_URL, &body).await {
        Ok(v) => v.get("result").cloned(),
        Err(_) => None,
    }
}

pub async fn chain_id(http: &reqwest::Client) -> u64 {
    match call(http, "eth_chainId", json!([])).await {
        Some(Value::String(hex)) => {
            u64::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(FALLBACK_CHAIN_ID)
        }
        _ => FALLBACK_CHAIN_ID,
    }
}

pub async fn balance_of(http: &reqwest::Client, address: &str) -> Option<f64> {
    match call(http, "eth_getBalance", json!([address, "latest"])).await {
        Some(Value::String(hex)) => {
            let wei = u128::from_str_radix(hex.trim_start_matches("0x"), 16).ok()?;
            Some(wei as f64 / 1e18)
        }
        _ => None,
    }
}

pub async fn pool_balance(http: &reqwest::Client) -> Option<f64> {
    let url = format!("{}/api/getFaucetStatus", crate::support::consts::BASE_URL);
    let rsp = client::post_json(http, &url, &json!({})).await.ok()?;
    let wei = rsp
        .get("status")?
        .get("walletBalance")?
        .as_str()?
        .parse::<u128>()
        .ok()?;
    Some(wei as f64 / 1e18)
}
