use crate::cuda::batch;
use crate::cuda::device::{self, DeviceInfo};
use crate::cuda::launch::LaunchParams;
use crate::cuda::loader::Library;
use crate::domain::params::PowParams;
use crate::mine::worker::{hash_nonce, FoundShare};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct GpuMiner {
    lib: Library,
    index: i32,
    name: String,
}

impl Clone for GpuMiner {
    fn clone(&self) -> Self {
        match GpuMiner::open(self.index) {
            Some(gpu) => gpu,
            None => Self {
                lib: self.lib.reopen(),
                index: self.index,
                name: self.name.clone(),
            },
        }
    }
}

impl GpuMiner {
    pub fn open(index: i32) -> Option<Self> {
        let lib = Library::load_any()?;
        let total = device::count(&lib);
        if total <= 0 || index >= total {
            return None;
        }
        if !device::select(&lib, index) {
            return None;
        }
        let name = device::name(&lib, index).unwrap_or_else(|| String::from("unnamed device"));
        Some(Self { lib, index, name })
    }

    pub fn index(&self) -> i32 {
        self.index
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn library(&self) -> &Library {
        &self.lib
    }

    pub fn devices(&self) -> Vec<DeviceInfo> {
        device::list(&self.lib)
    }

    pub fn runtime(&self) -> i32 {
        device::runtime_version(&self.lib).unwrap_or(0)
    }

    pub fn search(
        &self,
        params: &PowParams,
        preimage: &[u8],
        start: u64,
        count: u64,
        hashes: &AtomicU64,
    ) -> Vec<FoundShare> {
        device::select(&self.lib, self.index);

        let launch = LaunchParams::from_params(params);
        let mask = params.difficulty_mask();
        let nonces = match batch::scan(&self.lib, preimage, start, count, &launch, &mask) {
            Ok(nonces) => {
                hashes.fetch_add(count, Ordering::Relaxed);
                nonces
            }
            Err(code) => {
                crate::cuda::error::warn_once(&self.lib, code);
                return Vec::new();
            }
        };
        nonces
            .into_iter()
            .filter_map(|nonce| {
                let hash_hex = hash_nonce(params, preimage, nonce)?;
                Some(FoundShare { nonce, hash_hex })
            })
            .collect()
    }
}
