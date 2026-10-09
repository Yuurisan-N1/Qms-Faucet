use crate::domain::params::PowParams;
use crate::mine::worker::{hash_nonce, FoundShare};
use rayon::prelude::*;
use rayon::ThreadPool;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CpuMiner {
    threads: usize,
    pool: ThreadPool,
}

impl Clone for CpuMiner {
    fn clone(&self) -> Self {
        Self::new(self.threads)
    }
}

impl CpuMiner {
    pub fn new(threads: usize) -> Self {
        let resolved = resolve_threads(threads);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(resolved)
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().expect("thread pool"));
        Self {
            threads: resolved,
            pool,
        }
    }

    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn search(
        &self,
        params: &PowParams,
        preimage: &[u8],
        start: u64,
        count: u64,
        hashes: &AtomicU64,
    ) -> Vec<FoundShare> {
        let params_ref = params;
        let preimage_ref = preimage;
        self.pool.install(|| {
            (start..start + count)
                .into_par_iter()
                .filter_map(|nonce| {
                    hashes.fetch_add(1, Ordering::Relaxed);
                    let hash = hash_nonce(params_ref, preimage_ref, nonce)?;
                    Some(FoundShare {
                        nonce,
                        hash_hex: hash,
                    })
                })
                .collect()
        })
    }
}

pub fn resolve_threads(requested: usize) -> usize {
    if requested > 0 {
        return requested.min(256);
    }
    let logical = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(4);
    logical.clamp(1, 8)
}
