use std::fmt;

#[derive(Debug)]
pub enum QmsError {
    NoAccounts,
    Config(String),
    Network(String),
    Solver(String),
    Session(String),
    Faucet(String),
    Mining(String),
    Cuda(String),
    Stopped,
}

impl fmt::Display for QmsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QmsError::NoAccounts => write!(f, "no account lines were found in address.txt"),
            QmsError::Config(m) => write!(f, "{}", m),
            QmsError::Network(m) => write!(f, "{}", m),
            QmsError::Solver(m) => write!(f, "{}", m),
            QmsError::Session(m) => write!(f, "{}", m),
            QmsError::Faucet(m) => write!(f, "{}", m),
            QmsError::Mining(m) => write!(f, "{}", m),
            QmsError::Cuda(m) => write!(f, "{}", m),
            QmsError::Stopped => write!(f, "script stopped by user"),
        }
    }
}

impl std::error::Error for QmsError {}

impl From<anyhow::Error> for QmsError {
    fn from(e: anyhow::Error) -> Self {
        QmsError::Network(e.to_string())
    }
}

impl QmsError {
    pub fn code(&self) -> String {
        let raw = match self {
            QmsError::Session(m) | QmsError::Faucet(m) => m.as_str(),
            QmsError::Stopped => return String::from("script stopped"),
            QmsError::NoAccounts => return String::from("no accounts"),
            QmsError::Config(_) => return String::from("bad config"),
            QmsError::Solver(_) => return String::from("solver failed"),
            QmsError::Mining(_) => return String::from("mining failed"),
            QmsError::Cuda(_) => return String::from("cuda failed"),
            QmsError::Network(m) => return network_code(m),
        };
        let code = raw.trim();
        let code = code.strip_prefix("failedCode=").unwrap_or(code);
        let shaped = !code.is_empty()
            && code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if shaped {
            code.to_string()
        } else {
            String::from("request rejected")
        }
    }
}

fn network_code(message: &str) -> String {
    let raw = message.trim();
    if let Some(pos) = raw.find("status ") {
        let digits: String = raw[pos + 7..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !digits.is_empty() {
            return format!("HTTP_{}", digits);
        }
    }
    let lower = raw.to_ascii_lowercase();
    if lower.contains("timed out") {
        return String::from("TIMEOUT");
    }
    if lower.contains("connection failed") {
        return String::from("CONNECT_FAILED");
    }
    if lower.contains("could not be read") || lower.contains("unexpected body") {
        return String::from("BAD_BODY");
    }
    if lower.contains("proxy") {
        return String::from("BAD_PROXY");
    }
    if lower.contains("could not start") {
        return String::from("CLIENT_FAILED");
    }
    String::from("request failed")
}

pub type QmsResult<T> = Result<T, QmsError>;
