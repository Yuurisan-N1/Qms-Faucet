use crate::domain::params::{hash_is_valid, PowParams};
use crate::mine::argon2d;
use crate::mine::backend::Backend;
use crate::mine::cpu::CpuMiner;
use crate::support::log::ly;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub struct FoundShare {
    pub nonce: u64,
    pub hash_hex: String,
}

pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub fn hash_nonce(params: &PowParams, preimage: &[u8], nonce: u64) -> Option<String> {
    let secret = nonce.to_be_bytes();
    let out = argon2d::hash(params, &secret, preimage, params.version_raw);
    if out.is_empty() {
        return None;
    }
    Some(to_hex(&out))
}

pub fn search(
    params: &PowParams,
    preimage: &[u8],
    backend: &Backend,
    start: u64,
    count: u64,
    hashes: &AtomicU64,
) -> Vec<FoundShare> {
    let mask = params.difficulty_mask();
    let raw = match backend {
        Backend::Cpu(cpu) => cpu.search(params, preimage, start, count, hashes),
        Backend::Gpu(gpu) => gpu.search(params, preimage, start, count, hashes),
    };

    raw.into_iter()
        .filter(|share| hash_is_valid(&share.hash_hex, &mask))
        .collect()
}

const ACCELERATOR_STALL_LIMIT: u32 = 3;

pub fn run(
    params: &PowParams,
    preimage: &[u8],
    backend: &Backend,
    start_nonce: u64,
    stop: Arc<AtomicBool>,
    hashes: Arc<AtomicU64>,
    sink: Sender<FoundShare>,
) {
    let mut cursor = start_nonce.max(1);
    let mut cpu_fallback: Option<Backend> = None;
    let mut stalls = 0u32;

    while !stop.load(Ordering::Relaxed) {
        let active = cpu_fallback.as_ref().unwrap_or(backend);
        let step = active.batch_size().max(1);

        let before = hashes.load(Ordering::Relaxed);
        let batch = search(params, preimage, active, cursor, step, &hashes);
        for share in batch {
            if sink.blocking_send(share).is_err() {
                return;
            }
        }
        cursor += step;

        if cpu_fallback.is_some() || !matches!(active, Backend::Gpu(_)) {
            continue;
        }
        if hashes.load(Ordering::Relaxed) > before {
            stalls = 0;
            continue;
        }
        stalls += 1;
        if stalls >= ACCELERATOR_STALL_LIMIT {
            ly("The gpu accelerator stopped answering so the cpu backend takes over");
            cpu_fallback = Some(Backend::Cpu(CpuMiner::new(0)));
            stalls = 0;
        }
    }
}
