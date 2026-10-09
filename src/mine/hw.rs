use crate::cuda::device;
use crate::cuda::launch::LaunchParams;
use crate::domain::params::{hash_is_valid, PowParams};
use crate::mine::backend::Backend;
use crate::mine::cpu::CpuMiner;
use crate::mine::gpu::GpuMiner;
use crate::mine::worker::hash_nonce;
use crate::support::consts::SELF_TEST_SECONDS;
use crate::support::log::ly;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

const PROBE_PREIMAGE: &[u8] = b"qms accelerator self test";
const PROBE_SCAN: u64 = 256;

pub fn resolve(requested: &str, threads: usize) -> Backend {
    match requested {
        "cpu" => Backend::Cpu(CpuMiner::new(threads)),
        "gpu" => match GpuMiner::open(0) {
            Some(gpu) => Backend::Gpu(gpu),
            None => Backend::Cpu(CpuMiner::new(threads)),
        },
        _ => match GpuMiner::open(0) {
            Some(gpu) => Backend::Gpu(gpu),
            None => Backend::Cpu(CpuMiner::new(threads)),
        },
    }
}

pub fn probe() -> Option<GpuMiner> {
    GpuMiner::open(0)
}

pub fn probe_verified(params: &PowParams) -> Option<GpuMiner> {
    if std::env::var("QMS_FORCE_CPU").is_ok() {
        return None;
    }

    let gpu = GpuMiner::open(0)?;
    let probe = gpu.clone();
    let params = params.clone();
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let _ = tx.send(self_test(&probe, &params));
    });

    match rx.recv_timeout(Duration::from_secs(SELF_TEST_SECONDS)) {
        Ok(Ok(())) => Some(gpu),
        Ok(Err(why)) => {
            ly(&format!(
                "The gpu accelerator failed its self test ({}) so the cpu backend is used",
                why
            ));
            None
        }
        Err(_) => {
            ly(&format!(
                "The gpu accelerator did not answer its self test within {} seconds so the cpu backend is used",
                SELF_TEST_SECONDS
            ));
            None
        }
    }
}

fn self_test(gpu: &GpuMiner, params: &PowParams) -> Result<(), String> {
    let lib = gpu.library();
    device::select(lib, gpu.index());
    let launch = LaunchParams::from_params(params);

    for nonce in 1u64..=4 {
        let got = crate::cuda::hash::argon2d(lib, PROBE_PREIMAGE, nonce, &launch).map_err(|code| {
            format!(
                "a probe hash came back with {}",
                crate::cuda::error::message(lib, code)
            )
        })?;
        let want = crate::mine::argon2d::hash(
            params,
            &nonce.to_be_bytes(),
            PROBE_PREIMAGE,
            params.version_raw,
        );
        if got != want {
            return Err(format!("probe nonce {} hashed differently on the gpu", nonce));
        }
    }

    let mut easy = params.clone();
    easy.difficulty = 4;
    let mask = easy.difficulty_mask();

    let counter = AtomicU64::new(0);
    let on_gpu: BTreeSet<u64> = gpu
        .search(&easy, PROBE_PREIMAGE, 1, PROBE_SCAN, &counter)
        .into_iter()
        .map(|share| share.nonce)
        .collect();

    let mut on_cpu = BTreeSet::new();
    for nonce in 1..=PROBE_SCAN {
        if let Some(hash) = hash_nonce(&easy, PROBE_PREIMAGE, nonce) {
            if hash_is_valid(&hash, &mask) {
                on_cpu.insert(nonce);
            }
        }
    }

    if on_gpu != on_cpu {
        return Err(format!(
            "a probe scan matched {} nonces on the gpu and {} on the cpu",
            on_gpu.len(),
            on_cpu.len()
        ));
    }

    Ok(())
}

pub fn diagnose() -> &'static str {
    match crate::cuda::loader::Library::load_any() {
        Some(lib) => {
            if crate::cuda::device::count(&lib) <= 0 {
                "no cuda device was found so the cpu backend is used"
            } else {
                "the cuda device could not be used so the cpu backend is used"
            }
        }
        None => "no accelerator library found so the cpu backend is used",
    }
}
