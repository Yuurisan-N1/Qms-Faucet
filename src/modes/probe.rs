use crate::mine::cpu::CpuMiner;
use crate::mine::hw;
use crate::mine::worker;
use crate::support::error::QmsResult;
use crate::support::log::{lg, ly};
use std::time::Instant;

pub async fn execute() -> QmsResult<()> {
    let workers = crate::mine::cpu::resolve_threads(0);
    lg(&format!("Cpu benchmark uses {} threads", workers));

    match hw::probe() {
        Some(gpu) => {
            let devices = gpu.devices();
            lg(&format!(
                "Accelerator runtime version {} reports {} devices",
                gpu.runtime(),
                devices.len()
            ));
            for device in devices.iter() {
                lg(&format!("Device {} is named {}", device.index, device.name));
            }
            lg(&format!(
                "Selected accelerator device {} named {}",
                gpu.index(),
                gpu.name()
            ));
        }
        None => ly("No accelerator library answered so the cpu backend is used"),
    }

    let threads = crate::mine::cpu::resolve_threads(workers);
    let miner = CpuMiner::new(workers);
    let preimage = [0u8, 1, 2, 3, 4, 5, 6, 7];
    let params = crate::domain::params::PowParams::synthetic();
    let started = Instant::now();
    let counter = std::sync::atomic::AtomicU64::new(0);
    let found = worker::search(
        &params,
        &preimage,
        &crate::mine::backend::Backend::Cpu(miner),
        0,
        256,
        &counter,
    );
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let rate = 256.0 / elapsed;
    lg(&format!(
        "Measured {} worker threads at {} H/s",
        threads,
        rate as u64
    ));
    lg(&format!("Benchmark batch produced {} matching shares", found.len()));
    Ok(())
}
