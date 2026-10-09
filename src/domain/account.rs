use crate::support::error::{QmsError, QmsResult};
use std::fs;

#[derive(Clone, Debug)]
pub struct Account {
    pub address: String,
    pub index: usize,
}

pub fn load(path: &str) -> QmsResult<Vec<Account>> {
    let text = fs::read_to_string(path)
        .map_err(|e| QmsError::Config(format!("could not read {}: {}", path, e)))?;

    let mut accounts = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let address = line.split('|').next().unwrap_or(line).trim().to_string();
        if !is_address(&address) {
            continue;
        }
        accounts.push(Account {
            address,
            index: accounts.len(),
        });
    }

    if accounts.is_empty() {
        return Err(QmsError::NoAccounts);
    }
    Ok(accounts)
}

pub fn is_address(value: &str) -> bool {
    let body = match value.strip_prefix("0x") {
        Some(b) => b,
        None => return false,
    };
    if body.len() != 40 {
        return false;
    }
    if !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    !body.chars().all(|c| c == '0')
}
