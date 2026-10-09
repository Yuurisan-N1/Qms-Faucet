use crate::mine::worker;
use crate::support::cfg::Config;
use crate::support::error::{QmsError, QmsResult};
use crate::support::log::{lg, ly};
use serde_json::Value;

pub async fn execute(cfg: &Config) -> QmsResult<()> {
    let _ = cfg;
    let path = "assets/argon2d_vectors.json";
    let text = std::fs::read_to_string(path)
        .map_err(|e| QmsError::Config(format!("could not read {}: {}", path, e)))?;
    let root: Value = serde_json::from_str(&text)
        .map_err(|_| QmsError::Config(String::from("vector file is not valid json")))?;

    let vectors = root
        .get("vectors")
        .and_then(|v| v.as_array())
        .ok_or_else(|| QmsError::Config(String::from("vector file holds no vectors")))?;

    let mut passed = 0u64;
    let mut failed = 0u64;

    for vector in vectors {
        let preimage_hex = vector
            .get("preimage_hex")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let nonce = vector.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0);
        let version = vector.get("version").and_then(|v| v.as_u64()).unwrap_or(13) as u32;
        let expected = vector
            .get("hash_hex")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let preimage = match decode_hex(preimage_hex) {
            Some(bytes) => bytes,
            None => {
                failed += 1;
                continue;
            }
        };

        let mut params = crate::domain::params::PowParams::synthetic();
        params.version_raw = version;

        match worker::hash_nonce(&params, &preimage, nonce) {
            Some(hash) if hash == expected => passed += 1,
            Some(hash) => {
                failed += 1;
                ly(&format!(
                    "Vector with nonce {} expected {} but produced {}",
                    nonce, expected, hash
                ));
            }
            None => failed += 1,
        }
    }

    lg(&format!(
        "Self test finished with {} matches and {} mismatches",
        passed, failed
    ));

    if failed == 0 {
        lg("Argon2d parity with the faucet reference is confirmed");
        Ok(())
    } else {
        Err(QmsError::Mining(String::from(
            "self test found a hash mismatch against the reference vectors",
        )))
    }
}

pub fn decode_hex(input: &str) -> Option<Vec<u8>> {
    if input.len() % 2 != 0 {
        return None;
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(input.len() / 2);
    let mut index = 0;
    while index < bytes.len() {
        let hi = (bytes[index] as char).to_digit(16)?;
        let lo = (bytes[index + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        index += 2;
    }
    Some(out)
}
