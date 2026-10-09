use crate::support::error::QmsResult;
use std::fs;

#[derive(Clone, Debug)]
pub struct Proxy {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub pass: Option<String>,
}

impl Proxy {
    pub fn authority(&self) -> String {
        match (&self.user, &self.pass) {
            (Some(u), Some(p)) => format!("{}:{}@{}:{}", u, p, self.host, self.port),
            _ => format!("{}:{}", self.host, self.port),
        }
    }

    pub fn url(&self) -> String {
        format!("{}://{}", self.scheme, self.authority())
    }

    pub fn masked(&self) -> String {
        let octets: Vec<&str> = self.host.split('.').collect();
        if octets.len() == 4 {
            return format!(
                "http://user:pass@{}*****{}:{}",
                octets[0], octets[3], self.port
            );
        }
        if self.host.chars().count() > 4 {
            let first: String = self.host.chars().take(2).collect();
            let last: String = self
                .host
                .chars()
                .rev()
                .take(2)
                .collect::<Vec<char>>()
                .into_iter()
                .rev()
                .collect();
            return format!("http://user:pass@{first}*****{last}:{}", self.port);
        }
        String::from("http://user:pass@***:***")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Egress {
    proxies: Vec<Proxy>,
}

impl Egress {
    pub fn load(path: &str) -> QmsResult<Self> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return Ok(Self::default()),
        };

        let mut proxies = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(p) = parse_line(line) {
                proxies.push(p);
            } else {
                crate::support::log::ly(
                    "A proxy line was skipped because its format could not be read",
                );
            }
        }
        Ok(Self { proxies })
    }

    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }

    pub fn len(&self) -> usize {
        self.proxies.len()
    }

    pub fn duplicated(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.proxies
            .iter()
            .any(|p| !seen.insert(format!("{}:{}", p.host, p.port)))
    }

    pub fn pick(&self, index: usize) -> Option<&Proxy> {
        if self.proxies.is_empty() {
            None
        } else {
            Some(&self.proxies[index % self.proxies.len()])
        }
    }
}

fn parse_line(line: &str) -> Option<Proxy> {
    let (scheme, rest) = match line.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r.to_string()),
        None => ("socks5".to_string(), line.to_string()),
    };

    if let Some((creds, hostport)) = rest.rsplit_once('@') {
        let (user, pass) = creds.split_once(':').unwrap_or((creds, ""));
        let (host, port) = hostport.rsplit_once(':')?;
        return Some(Proxy {
            scheme,
            host: host.to_string(),
            port: port.parse().ok()?,
            user: Some(user.to_string()),
            pass: Some(pass.to_string()),
        });
    }

    let parts: Vec<&str> = rest.split(':').collect();
    match parts.len() {
        2 => Some(Proxy {
            scheme,
            host: parts[0].to_string(),
            port: parts[1].parse().ok()?,
            user: None,
            pass: None,
        }),
        4 => Some(Proxy {
            scheme,
            host: parts[0].to_string(),
            port: parts[1].parse().ok()?,
            user: Some(parts[2].to_string()),
            pass: Some(parts[3].to_string()),
        }),
        _ => None,
    }
}
