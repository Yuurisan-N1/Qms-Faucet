use crate::support::error::{QmsError, QmsResult};

#[derive(Clone, Debug)]
pub struct PowParams {
    pub algo: String,
    pub type_cost: u32,
    pub version_raw: u32,
    pub time_cost: u32,
    pub mem_cost: u32,
    pub parallelism: u32,
    pub key_length: u32,
    pub difficulty: u32,
    pub hashrate_limit: u64,
}

impl PowParams {
    pub fn from_config(cfg: &serde_json::Value) -> QmsResult<Self> {
        let pow = cfg
            .get("modules")
            .and_then(|m| m.get("pow"))
            .ok_or_else(|| QmsError::Faucet("faucet did not report a mining module".to_string()))?;

        let p = pow
            .get("powParams")
            .ok_or_else(|| QmsError::Faucet("faucet mining parameters were missing".to_string()))?;

        let algo = p
            .get("a")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if algo != "argon2" {
            return Err(QmsError::Faucet(format!(
                "faucet asked for an unsupported algorithm {}",
                algo
            )));
        }

        let n = |key: &str, dflt: u64| p.get(key).and_then(|v| v.as_u64()).unwrap_or(dflt) as u32;

        Ok(Self {
            algo,
            type_cost: n("t", 0),
            version_raw: n("v", 13),
            time_cost: n("i", 4),
            mem_cost: n("m", 4096),
            parallelism: n("p", 1),
            key_length: n("l", 16),
            difficulty: pow
                .get("powDifficulty")
                .and_then(|v| v.as_u64())
                .unwrap_or(11) as u32,
            hashrate_limit: pow
                .get("powHashrateLimit")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })
    }

    pub fn synthetic() -> Self {
        Self {
            algo: String::from("argon2"),
            type_cost: 0,
            version_raw: 13,
            time_cost: 4,
            mem_cost: 4096,
            parallelism: 1,
            key_length: 16,
            difficulty: 11,
            hashrate_limit: 0,
        }
    }

    pub fn params_str(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            self.algo,
            self.type_cost,
            self.version_raw,
            self.time_cost,
            self.mem_cost,
            self.parallelism,
            self.key_length,
            self.difficulty
        )
    }

    pub fn difficulty_mask(&self) -> String {
        let byte_count = self.difficulty / 8 + 1;
        let bit_count = self.difficulty - ((byte_count - 1) * 8);
        let max_value = 1u32 << (8 - bit_count);
        let mut mask = format!("{:x}", max_value);
        while mask.len() < (byte_count as usize) * 2 {
            mask.insert(0, '0');
        }
        mask
    }

    pub fn expected_shares_per_hash(&self) -> u64 {
        1u64 << self.difficulty
    }
}

pub fn hash_is_valid(hash_hex: &str, mask: &str) -> bool {
    if hash_hex.len() < mask.len() {
        return false;
    }
    &hash_hex[..mask.len()] <= mask
}
