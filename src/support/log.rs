use std::io::{IsTerminal, Write};
use std::sync::atomic::Ordering as O;

const G: &str = "\x1b[1;32m";
const Y: &str = "\x1b[1;33m";
const R: &str = "\x1b[1;31m";
const Z: &str = "\x1b[0m";

fn plain(color: &str, msg: &str) {
    println!("{}{}{}", color, msg, Z);
    let _ = std::io::stdout().flush();
}

pub fn lg(msg: &str) {
    line(G, msg);
}

pub fn ly(msg: &str) {
    line(Y, msg);
}

pub fn lr(msg: &str) {
    line(R, msg);
}

fn line(color: &str, msg: &str) {
    if !countdown_visible() {
        plain(color, msg);
        return;
    }
    let _guard = match LIVE_LOCK.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let prev = LIVE_ROWS.load(O::Relaxed);
    erase_block(prev);
    LIVE_ROWS.store(0, O::Relaxed);
    plain(color, msg);
    draw_live();
}

pub fn gap() {
    if !countdown_visible() {
        println!();
        let _ = std::io::stdout().flush();
        return;
    }
    let _guard = match LIVE_LOCK.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let prev = LIVE_ROWS.load(O::Relaxed);
    erase_block(prev);
    LIVE_ROWS.store(0, O::Relaxed);
    println!();
    draw_live();
}

pub fn clean_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut sp = false;
    for c in input.chars() {
        if c.is_control() || c == '\u{feff}' {
            if c == '\n' || c == '\r' || c == '\t' {
                sp = true;
            }
            continue;
        }
        if c.is_whitespace() {
            sp = true;
            continue;
        }
        if sp && !out.is_empty() {
            out.push(' ');
        }
        sp = false;
        out.push(c);
    }
    out
}

pub fn short(input: &str, max: usize) -> String {
    let s = clean_text(input);
    if s.chars().count() <= max {
        return s;
    }
    let cut: String = s.chars().take(max + 1).collect();
    match cut.rfind(' ') {
        Some(i) if i > 0 => cut[..i].trim_end().to_string(),
        _ => s.chars().take(max).collect(),
    }
}

static BACKEND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static COIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_backend(text: &str) {
    let _ = BACKEND.set(text.to_string());
}

pub fn set_coin(text: &str) {
    let _ = COIN.set(text.to_string());
}

pub fn coin_symbol() -> String {
    COIN.get().cloned().unwrap_or_else(|| String::from("QMS"))
}

pub fn short_device(name: &str) -> String {
    let mut text = name.to_string();
    for prefix in ["NVIDIA GeForce ", "GeForce ", "NVIDIA ", "AMD Radeon ", "Radeon ", "AMD "] {
        text = text.replace(prefix, "");
    }
    let trimmed: String = text.trim().chars().take(26).collect();
    trimmed.trim().to_string()
}

pub fn unit_word(value: i64, singular: &str, plural: &str) -> String {
    if value == 1 {
        singular.to_string()
    } else {
        plural.to_string()
    }
}

pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let rest = seconds % 60;
    if hours > 0 {
        format!(
            "{} {} {} {}",
            hours,
            unit_word(hours as i64, "hour", "hours"),
            minutes,
            unit_word(minutes as i64, "minute", "minutes")
        )
    } else if minutes > 0 {
        format!(
            "{} {} {} {}",
            minutes,
            unit_word(minutes as i64, "minute", "minutes"),
            rest,
            unit_word(rest as i64, "second", "seconds")
        )
    } else {
        format!("{} {}", rest, unit_word(rest as i64, "second", "seconds"))
    }
}

pub fn countdown_visible() -> bool {
    std::io::stdout().is_terminal()
}

const DEFAULT_LANE: &str = "main";
const CD_KEY: &str = "countdown";

struct Row {
    key: String,
    text: String,
    color: &'static str,
}

static LIVE_ROWS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static LIVE_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LIVE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn rows() -> &'static std::sync::Mutex<Vec<Row>> {
    static ROWS: std::sync::OnceLock<std::sync::Mutex<Vec<Row>>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn lane_key(lane: Option<&str>) -> String {
    clean_text(lane.unwrap_or(DEFAULT_LANE))
}

fn set_row(key: &str, text: &str, color: &'static str) -> bool {
    match rows().lock() {
        Ok(mut list) => {
            for row in list.iter_mut() {
                if row.key == key {
                    let changed = row.text != text || row.color != color;
                    row.text = text.to_string();
                    row.color = color;
                    return changed;
                }
            }
            list.push(Row {
                key: key.to_string(),
                text: text.to_string(),
                color,
            });
            true
        }
        Err(_) => false,
    }
}

fn drop_row(key: &str) -> bool {
    match rows().lock() {
        Ok(mut list) => {
            let before = list.len();
            list.retain(|row| row.key != key);
            before != list.len()
        }
        Err(_) => false,
    }
}

fn erase_block(rows_drawn: usize) {
    if rows_drawn == 0 || !countdown_visible() {
        return;
    }
    let mut out = std::io::stdout();
    let _ = write!(out, "\x1b[{}F\x1b[J", rows_drawn);
    let _ = out.flush();
}

fn draw_live() {
    let prev = LIVE_ROWS.load(O::Relaxed);
    let list = match rows().lock() {
        Ok(v) => v,
        Err(_) => return,
    };
    if list.is_empty() {
        erase_block(prev);
        LIVE_ROWS.store(0, O::Relaxed);
        LIVE_OPEN.store(false, O::Relaxed);
        return;
    }

    let active = list.iter().filter(|r| r.key != CD_KEY).count();
    let mut out = String::new();
    if prev > 0 {
        out.push_str(&format!("\x1b[{}F", prev));
    }
    let mut drawn = 0;
    if active > 0 {
        let accel = BACKEND.get().map(|v| v.as_str()).unwrap_or("auto");
        let head = format!(
            "Mining on {} with {} {} now",
            accel,
            active,
            unit_word(active as i64, "wallet", "wallets")
        );
        out.push_str("\x1b[2K\r");
        out.push_str(G);
        out.push_str(&head);
        out.push_str(Z);
        out.push('\n');
        drawn += 1;
    }
    for row in list.iter().filter(|r| r.key != CD_KEY) {
        out.push_str("\x1b[2K\r");
        out.push_str(row.color);
        out.push_str(&row.text);
        out.push_str(Z);
        out.push('\n');
        drawn += 1;
    }
    for row in list.iter().filter(|r| r.key == CD_KEY) {
        out.push_str("\x1b[2K\r");
        out.push_str(row.color);
        out.push_str(&row.text);
        out.push_str(Z);
        out.push('\n');
        drawn += 1;
    }
    if drawn < prev {
        out.push_str("\x1b[J");
    }
    LIVE_ROWS.store(drawn, O::Relaxed);
    LIVE_OPEN.store(true, O::Relaxed);
    let mut out_stream = std::io::stdout();
    let _ = write!(out_stream, "{}", out);
    let _ = out_stream.flush();
}

fn render_live() {
    if let Ok(_guard) = LIVE_LOCK.lock() {
        draw_live();
    }
}

pub fn close_live() {
    let _guard = match LIVE_LOCK.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let prev = LIVE_ROWS.swap(0, O::Relaxed);
    LIVE_OPEN.store(false, O::Relaxed);
    if let Ok(mut list) = rows().lock() {
        list.clear();
    }
    erase_block(prev);
}

pub fn live(msg: &str) {
    live_for(None, msg);
}

pub fn live_for(lane: Option<&str>, msg: &str) {
    let text = clean_text(msg);
    let key = lane_key(lane);
    if !countdown_visible() {
        if !set_row(&key, &text, G) {
            return;
        }
        plain(G, &text);
        return;
    }
    if !set_row(&key, &text, G) {
        return;
    }
    render_live();
}

pub fn live_end() {
    if let Ok(mut list) = rows().lock() {
        list.clear();
    }
}

pub fn live_end_for(lane: Option<&str>) {
    let key = lane_key(lane);
    if !drop_row(&key) {
        return;
    }
    render_live();
}

pub async fn wait_seconds(seconds: u64) {
    for _ in 0..seconds {
        if crate::support::shutdown::requested() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub async fn countdown(seconds: u64, label: &str) {
    let text = clean_text(label);
    if seconds == 0 {
        return;
    }
    if !countdown_visible() {
        wait_seconds(seconds).await;
        return;
    }
    let mut left = seconds;
    while left > 0 {
        if crate::support::shutdown::requested() {
            break;
        }
        let days = left / 86400;
        let hours = (left % 86400) / 3600;
        let minutes = (left % 3600) / 60;
        let rest = left % 60;
        let shown = if days > 0 {
            format!("{} {} {:02}:{:02}:{:02}", text, days, hours, minutes, rest)
        } else {
            format!("{} {:02}:{:02}:{:02}", text, hours, minutes, rest)
        };
        if set_row(CD_KEY, &shown, Y) {
            render_live();
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        left -= 1;
    }
    if drop_row(CD_KEY) {
        render_live();
    }
}
