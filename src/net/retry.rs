use std::time::Duration;

pub fn backoff_ms(attempt: u32, base_ms: u64, cap_ms: u64) -> u64 {
    let shift = attempt.min(6);
    let value = base_ms.saturating_mul(1u64 << shift);
    value.min(cap_ms)
}

pub async fn pause(attempt: u32, base_ms: u64, cap_ms: u64) {
    let ms = backoff_ms(attempt, base_ms, cap_ms);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
