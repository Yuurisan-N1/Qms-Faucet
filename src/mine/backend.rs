use crate::mine::cpu::CpuMiner;
use crate::mine::gpu::GpuMiner;

#[derive(Clone)]
pub enum Backend {
    Cpu(CpuMiner),
    Gpu(GpuMiner),
}

impl Backend {
    pub fn label(&self) -> &'static str {
        match self {
            Backend::Cpu(_) => "cpu",
            Backend::Gpu(_) => "gpu",
        }
    }

    pub fn batch_size(&self) -> u64 {
        match self {
            Backend::Cpu(_) => crate::support::consts::CPU_BATCH,
            Backend::Gpu(_) => crate::support::consts::GPU_BATCH,
        }
    }

    pub fn short_name(&self) -> String {
        match self {
            Backend::Cpu(cpu) => format!("{} threads", cpu.threads()),
            Backend::Gpu(gpu) => crate::support::log::short_device(gpu.name()),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Backend::Cpu(cpu) => format!("{} threads", cpu.threads()),
            Backend::Gpu(gpu) => format!("device {} named {}", gpu.index(), gpu.name()),
        }
    }
}
