use crate::net::egress::Proxy;
use crate::support::error::{QmsError, QmsResult};
use base64::Engine as _;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct WsSession {
    sink: SplitSink<Ws, Message>,
    stream: SplitStream<Ws>,
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    )
}

impl WsSession {
    pub async fn connect(session_id: &str, proxy: Option<&Proxy>) -> QmsResult<Self> {
        let host = crate::support::consts::BASE_URL
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let url = format!(
            "wss://{}/ws/pow?session={}&cliver=2.5.1",
            host, session_id
        );

        let mut request = url
            .into_client_request()
            .map_err(|e| QmsError::Network(format!("socket request was rejected: {}", e)))?;
        {
            let headers = request.headers_mut();
            headers.insert("origin", crate::support::consts::BASE_URL.parse().unwrap());
            headers.insert("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36".parse().unwrap());
        }

        let ws = match proxy {
            Some(p) => {
                let stream = open_stream(host, 443, proxy).await?;
                let connector = tokio_tungstenite::Connector::Rustls(tls_config());
                let (ws, _rsp) = tokio_tungstenite::client_async_tls_with_config(
                    request,
                    stream,
                    None,
                    Some(connector),
                )
                .await
                .map_err(|e| QmsError::Network(format!("socket handshake failed: {}", e)))?;
                let _ = p;
                ws
            }
            _ => {
                let (ws, _rsp) = tokio_tungstenite::connect_async(request)
                    .await
                    .map_err(|e| QmsError::Network(format!("socket handshake failed: {}", e)))?;
                ws
            }
        };

        let (sink, stream) = ws.split();
        Ok(Self { sink, stream })
    }

    pub async fn send_share(
        &mut self,
        req_id: u64,
        nonce: u64,
        hash_hex: &str,
        params_str: &str,
        hashrate: f64,
    ) -> QmsResult<()> {
        let payload = json!({
            "action": "foundShare",
            "id": req_id,
            "data": {
                "nonce": nonce,
                "data": hash_hex,
                "params": params_str,
                "hashrate": hashrate,
            }
        });
        self.send(payload).await
    }

    pub async fn send_verify_result(
        &mut self,
        share_id: &str,
        params_str: &str,
        is_valid: bool,
    ) -> QmsResult<()> {
        let payload = json!({
            "action": "verifyResult",
            "data": {
                "shareId": share_id,
                "params": params_str,
                "isValid": is_valid,
            }
        });
        self.send(payload).await
    }

    pub async fn send_close(&mut self, req_id: u64) -> QmsResult<()> {
        let payload = json!({
            "action": "closeSession",
            "id": req_id,
        });
        self.send(payload).await
    }

    async fn send(&mut self, payload: Value) -> QmsResult<()> {
        let text = payload.to_string();
        self.sink
            .send(Message::Text(text))
            .await
            .map_err(|e| QmsError::Network(format!("socket send failed: {}", e)))
    }

    pub async fn next_message(&mut self) -> QmsResult<Option<Value>> {
        loop {
            let msg = match self.stream.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => {
                    return Err(QmsError::Network(format!("socket read failed: {}", e)))
                }
                None => return Ok(None),
            };
            match msg {
                Message::Text(t) => {
                    if let Ok(v) = serde_json::from_str::<Value>(&t) {
                        return Ok(Some(v));
                    }
                }
                Message::Binary(b) => {
                    if let Ok(v) = serde_json::from_slice::<Value>(&b) {
                        return Ok(Some(v));
                    }
                }
                Message::Ping(p) => {
                    let _ = self.sink.send(Message::Pong(p)).await;
                }
                Message::Close(_) => return Ok(None),
                _ => {}
            }
        }
    }
}

async fn open_stream(
    host: &str,
    port: u16,
    proxy: Option<&Proxy>,
) -> QmsResult<TcpStream> {
    let stream = match proxy {
        Some(p) if p.scheme.starts_with("socks") => {
            let target = format!("{}:{}", host, port);
            let proxy_addr = format!("{}:{}", p.host, p.port);
            let s = match (&p.user, &p.pass) {
                (Some(u), Some(pw)) => {
                    tokio_socks::tcp::Socks5Stream::connect_with_password(
                        proxy_addr.as_str(),
                        target.as_str(),
                        u.as_str(),
                        pw.as_str(),
                    )
                    .await
                }
                _ => {
                    tokio_socks::tcp::Socks5Stream::connect(
                        proxy_addr.as_str(),
                        target.as_str(),
                    )
                    .await
                }
            }
            .map_err(|e| QmsError::Network(format!("proxy tunnel failed: {}", e)))?;
            s.into_inner()
        }
        Some(p) if p.scheme.starts_with("http") => http_tunnel(host, port, p).await?,
        _ => TcpStream::connect(format!("{}:{}", host, port))
            .await
            .map_err(|e| QmsError::Network(format!("socket connect failed: {}", e)))?,
    };
    Ok(stream)
}

async fn http_tunnel(host: &str, port: u16, p: &Proxy) -> QmsResult<TcpStream> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut sock = TcpStream::connect(format!("{}:{}", p.host, p.port))
        .await
        .map_err(|e| QmsError::Network(format!("proxy connect failed: {}", e)))?;
    let mut req = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
        host, port, host, port
    );
    if let (Some(u), Some(pw)) = (&p.user, &p.pass) {
        let cred = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", u, pw));
        req.push_str(&format!("Proxy-Authorization: Basic {}\r\n", cred));
    }
    req.push_str("Proxy-Connection: Keep-Alive\r\n\r\n");
    sock.write_all(req.as_bytes())
        .await
        .map_err(|e| QmsError::Network(format!("proxy write failed: {}", e)))?;

    let mut head = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = sock
            .read(&mut byte)
            .await
            .map_err(|e| QmsError::Network(format!("proxy read failed: {}", e)))?;
        if n == 0 {
            return Err(QmsError::Network(String::from("proxy closed the tunnel")));
        }
        head.push(byte[0]);
        if head.len() >= 4 && &head[head.len() - 4..] == b"\r\n\r\n" {
            break;
        }
        if head.len() > 8192 {
            return Err(QmsError::Network(String::from("proxy reply was too large")));
        }
    }
    let line = String::from_utf8_lossy(&head);
    let status = line.lines().next().unwrap_or("").to_string();
    if !status.contains(" 200") {
        return Err(QmsError::Network(format!("proxy tunnel refused: {}", status)));
    }
    Ok(sock)
}
