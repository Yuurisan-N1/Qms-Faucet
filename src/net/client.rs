use crate::net::egress::Proxy;
use crate::support::error::{QmsError, QmsResult};
use reqwest::{Client, Response};
use std::time::Duration;

pub fn build(proxy: Option<&Proxy>) -> QmsResult<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
        );

    if let Some(p) = proxy {
        let scheme = if p.scheme.starts_with("http") {
            "http"
        } else {
            "socks5h"
        };
        let mut url = format!("{}://", scheme);
        if let (Some(u), Some(pw)) = (&p.user, &p.pass) {
            url.push_str(&encode_userinfo(u));
            url.push(':');
            url.push_str(&encode_userinfo(pw));
            url.push('@');
        }
        url.push_str(&p.host);
        url.push(':');
        url.push_str(&p.port.to_string());
        let parsed = reqwest::Proxy::all(&url).map_err(|e| {
            QmsError::Network(format!(
                "proxy {} could not be parsed: {}",
                p.masked(),
                e
            ))
        })?;
        builder = builder.proxy(parsed);
    }

    builder
        .build()
        .map_err(|e| QmsError::Network(format!("http client could not start: {}", e)))
}

fn net_reason(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        String::from("the request timed out")
    } else if e.is_connect() {
        String::from("the connection failed")
    } else if e.is_body() || e.is_decode() {
        String::from("the reply could not be read")
    } else {
        String::from("the request failed")
    }
}

fn encode_userinfo(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub async fn get_json(client: &Client, url: &str) -> QmsResult<serde_json::Value> {
    let rsp = client
        .get(url)
        .header("accept", "application/json, text/plain")
        .header("referer", crate::support::consts::BASE_URL)
        .send()
        .await
        .map_err(|e| QmsError::Network(net_reason(&e)))?;
    read_json(rsp).await
}

pub async fn post_json(
    client: &Client,
    url: &str,
    body: &serde_json::Value,
) -> QmsResult<serde_json::Value> {
    let rsp = client
        .post(url)
        .header("content-type", "application/json")
        .header("origin", crate::support::consts::BASE_URL)
        .header("referer", format!("{}/", crate::support::consts::BASE_URL))
        .json(body)
        .send()
        .await
        .map_err(|e| QmsError::Network(net_reason(&e)))?;
    read_json(rsp).await
}

async fn read_json(rsp: Response) -> QmsResult<serde_json::Value> {
    let status = rsp.status();
    let text = rsp
        .text()
        .await
        .map_err(|e| QmsError::Network(net_reason(&e)))?;
    if !status.is_success() {
        if let Some(code) = failed_code_of(&text) {
            return Err(QmsError::Faucet(format!("failedCode={}", code)));
        }
        return Err(QmsError::Network(format!(
            "the server answered with status {}",
            status.as_u16()
        )));
    }
    serde_json::from_str(&text)
        .map_err(|_| QmsError::Network("server answered with an unexpected body".to_string()))
}

fn failed_code_of(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let code = value.get("failedCode").and_then(|v| v.as_str())?;
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}
