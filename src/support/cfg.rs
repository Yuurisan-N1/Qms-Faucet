use crate::support::error::{QmsError, QmsResult};
use serde_json::Value;
use std::fs;

#[derive(Clone, Debug)]
pub struct Config {
    raw: Value,
}

const DEFAULTS: &str = r#"{
  "settings": {
    "sleep_seconds": 3600,
    "workers": 0,
    "target_qms": 1.0
  }
}
"#;

impl Config {
    pub fn load(path: &str) -> QmsResult<Self> {
        if !std::path::Path::new(path).exists() {
            fs::write(path, DEFAULTS)
                .map_err(|e| QmsError::Config(format!("could not write default config: {}", e)))?;
            crate::support::log::ly(&format!("{} was missing so defaults were written", path));
        }
        let text = fs::read_to_string(path)
            .map_err(|e| QmsError::Config(format!("could not read {}: {}", path, e)))?;
        let raw: Value = serde_json::from_str(&text)
            .map_err(|_| QmsError::Config(format!("{} is not valid json", path)))?;
        Ok(Self { raw })
    }

    fn top(&self, key: &str) -> Value {
        self.raw.get(key).cloned().unwrap_or(Value::Null)
    }

    fn nested(&self, module: &str, key: &str) -> Value {
        self.raw
            .get(module)
            .and_then(|m| m.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn number(&self, key: &str, module: &str) -> Option<f64> {
        self.top(key)
            .as_f64()
            .or_else(|| self.nested(module, key).as_f64())
    }

    pub fn sleep_seconds(&self) -> u64 {
        self.number("sleep_seconds", "settings")
            .filter(|v| *v > 0.0)
            .map(|v| v as u64)
            .unwrap_or(3600)
    }

    pub fn workers(&self) -> usize {
        let n = self
            .number("workers", "settings")
            .or_else(|| self.number("threads", "settings"))
            .unwrap_or(0.0);
        if n <= 0.0 {
            0
        } else {
            (n as usize).min(256)
        }
    }

    pub fn target_qms(&self) -> f64 {
        self.number("target_qms", "settings")
            .filter(|v| *v > 0.0)
            .unwrap_or(0.0)
    }
}
