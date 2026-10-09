use crate::net::client;
use crate::support::consts::{BASE_URL, CLAIM_POLL_ATTEMPTS, CLAIM_POLL_SECONDS};
use crate::support::error::{QmsError, QmsResult};
use crate::support::log::{countdown, lg, ly};
use serde_json::json;

fn status_url(session: &str) -> String {
    format!(
        "{}/api/getSessionStatus?session={}&details=1",
        BASE_URL, session
    )
}

fn text_of(state: &serde_json::Value, key: &str) -> String {
    state
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn balance_of(state: &serde_json::Value, fallback: f64) -> f64 {
    let wei = state
        .get("balance")
        .and_then(|v| v.as_str())
        .and_then(|x| x.parse::<u128>().ok())
        .unwrap_or(0);
    if wei > 0 {
        wei as f64 / 1e18
    } else {
        fallback
    }
}

async fn wait_claimable(
    http: &reqwest::Client,
    url: &str,
    mined: f64,
    min_claim: f64,
) -> QmsResult<Option<f64>> {
    let mut balance = mined;
    for attempt in 1..=CLAIM_POLL_ATTEMPTS {
        if let Ok(state) = client::get_json(http, url).await {
            balance = balance_of(&state, balance);
            match text_of(&state, "status").as_str() {
                "claimable" => return Ok(Some(balance)),
                "failed" => {
                    let code = state
                        .get("failedCode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("FAILED");
                    if code == "AMOUNT_TOO_LOW" {
                        ly(&format!(
                            "Session balance {:.2} is below the {:.2} payout minimum",
                            balance, min_claim
                        ));
                        return Ok(None);
                    }
                    return Err(QmsError::Faucet(format!("claim rejected with {}", code)));
                }
                _ => {}
            }
        }
        if attempt < CLAIM_POLL_ATTEMPTS {
            countdown(CLAIM_POLL_SECONDS, "Waiting until the session is claimable").await;
        }
    }
    Ok(None)
}

pub async fn claim(
    http: &reqwest::Client,
    session: &str,
    mined: f64,
    min_claim: f64,
) -> QmsResult<f64> {
    let url = status_url(session);

    let balance = match wait_claimable(http, &url, mined, min_claim).await? {
        Some(b) => b,
        None => {
            ly("Faucet payout is still pending so the next cycle checks again");
            return Ok(0.0);
        }
    };

    let target = format!("{}/api/claimReward", BASE_URL);
    let rsp = client::post_json(http, &target, &json!({ "session": session })).await?;
    if let Some(code) = rsp.get("failedCode").and_then(|v| v.as_str()) {
        if !code.is_empty() {
            return Err(QmsError::Faucet(format!("claim rejected with {}", code)));
        }
    }

    for attempt in 1..=CLAIM_POLL_ATTEMPTS {
        if let Ok(state) = client::get_json(http, &url).await {
            let claim = text_of(&state, "claimStatus");
            let amount = balance_of(&state, balance);
            if claim == "confirmed" || claim == "claimed" {
                lg(&format!("Faucet confirmed a payout of {} coins", amount));
                return Ok(amount);
            }
            if claim == "failed" {
                return Err(QmsError::Faucet(String::from(
                    "the faucet reported a failed payout",
                )));
            }
        }
        if attempt < CLAIM_POLL_ATTEMPTS {
            countdown(CLAIM_POLL_SECONDS, "Next task in").await;
        }
    }

    lg(&format!("Faucet queued a payout of {} coins", balance));
    Ok(balance)
}
