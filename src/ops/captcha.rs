use crate::net::client;
use crate::support::consts::{
    BASE_URL, SOLVE_ATTEMPTS, SOLVE_BUSY_SECONDS, SOLVE_BUSY_TRIES, SOLVE_RETRY_SECONDS,
    SOLVER_POLL_SECONDS, SOLVER_POLL_TRIES, SOLVER_URL, TURNSTILE_ACTION, TURNSTILE_SITEKEY,
};
use crate::support::error::{QmsError, QmsResult};
use crate::support::log::{countdown, lg, ly, wait_seconds};
use crate::support::shutdown;

static SOLVER_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

fn solver_lock() -> &'static tokio::sync::Mutex<()> {
    SOLVER_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn busy(e: &QmsError) -> bool {
    let text = e.to_string();
    text.contains("status 429") || text.contains("status 503")
}

pub async fn solve() -> QmsResult<String> {
    let _serial = solver_lock().lock().await;
    let http = client::build(None)?;
    let base = SOLVER_URL.trim_end_matches('/');
    let mut last = String::from("the solver never answered");

    let mut attempt: u32 = 1;
    let mut busy_waits: u32 = 0;

    while attempt <= SOLVE_ATTEMPTS {
        if shutdown::requested() {
            return Err(QmsError::Solver(String::from("stop was requested")));
        }
        let create = format!(
            "{}/turnstile?url={}&sitekey={}&action={}",
            base,
            encode(&format!("{}/", BASE_URL)),
            encode(TURNSTILE_SITEKEY),
            encode(TURNSTILE_ACTION),
        );

        match client::get_json(&http, &create).await {
            Ok(v) => {
                let task_id = v
                    .get("task_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                match task_id {
                    Some(id) => {
                        lg(&format!("Captcha task {} was accepted by the solver", id));
                        match poll(&http, base, &id).await {
                            Ok(token) => return Ok(token),
                            Err(e) => {
                                if busy(&e) {
                                    busy_waits += 1;
                                    if busy_waits > SOLVE_BUSY_TRIES {
                                        last = e.to_string();
                                        break;
                                    }
                                    wait_seconds(SOLVE_BUSY_SECONDS).await;
                                    continue;
                                }
                                last = e.to_string();
                            }
                        }
                    }
                    None => last = String::from("the solver answered without a task id"),
                }
            }
            Err(e) => {
                if busy(&e) {
                    busy_waits += 1;
                    if busy_waits > SOLVE_BUSY_TRIES {
                        last = e.to_string();
                        break;
                    }
                    wait_seconds(SOLVE_BUSY_SECONDS).await;
                    continue;
                }
                last = e.to_string();
            }
        }

        if attempt < SOLVE_ATTEMPTS {
            if shutdown::requested() {
                break;
            }
            ly(&format!(
                "Captcha attempt number {} did not succeed so it is retried",
                attempt
            ));
            countdown(SOLVE_RETRY_SECONDS, "Retry in").await;
        }
        attempt += 1;
    }

    if shutdown::requested() {
        return Err(QmsError::Solver(String::from("stop was requested")));
    }

    Err(QmsError::Solver(format!(
        "captcha could not be solved because {}",
        last
    )))
}

async fn poll(http: &reqwest::Client, base: &str, task_id: &str) -> QmsResult<String> {
    let url = format!("{}/result?id={}", base, task_id);
    for _ in 1..=SOLVER_POLL_TRIES {
        if shutdown::requested() {
            return Err(QmsError::Solver(String::from("stop was requested")));
        }
        match client::get_json(http, &url).await {
            Ok(v) => {
                let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("");
                match status {
                    "success" | "done" | "completed" => {
                        if let Some(tok) = v.get("value").and_then(|x| x.as_str()) {
                            if !tok.is_empty() {
                                return Ok(tok.to_string());
                            }
                        }
                        return Err(QmsError::Solver(
                            "the solver reported success without a token".to_string(),
                        ));
                    }
                    "process" | "processing" | "accepted" | "pending" => {}
                    other => {
                        return Err(QmsError::Solver(format!(
                            "the solver reported status {}",
                            if other.is_empty() { "unknown" } else { other }
                        )))
                    }
                }
            }
            Err(e) => {
                if busy(&e) {
                    return Err(e);
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(SOLVER_POLL_SECONDS)).await;
    }
    Err(QmsError::Solver(
        "the solver did not finish in time".to_string(),
    ))
}
