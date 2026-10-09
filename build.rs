
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Failed to attach Windows icon: {}", e);
        }
    }

    build_cuda();
}

fn root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()))
}

fn lib_name() -> &'static str {
    if cfg!(windows) {
        "qms_cuda.dll"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "libqms_cuda.dylib"
    } else {
        "libqms_cuda.so"
    }
}

fn build_cuda() {
    let root = root();
    let src = root.join("cuda").join("kernels").join("argon2_kernel.cu");
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed=cuda/include");
    println!("cargo:rerun-if-changed=cuda/kernels");

    if std::env::var("QMS_SKIP_CUDA").is_ok() {
        println!("cargo:warning=cuda build skipped on request, the cpu backend will be used");
        return;
    }
    if !src.exists() {
        println!("cargo:warning=cuda sources are missing, the cpu backend will be used");
        return;
    }

    let nvcc = match pick_nvcc() {
        Some(v) => v,
        None => {
            println!("cargo:warning=no nvcc found, the cpu backend will be used");
            println!("cargo:warning=install the cuda toolkit and rebuild to unlock the gpu");
            return;
        }
    };

    let major = nvcc_major(&nvcc);
    let arch = pick_arch();
    let out = root.join("cuda").join(lib_name());
    let cuda_home = nvcc
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/usr/local/cuda"));

    let libdir = if cfg!(windows) {
        cuda_home.join("lib").join("x64")
    } else {
        let l64 = cuda_home.join("lib64");
        if l64.is_dir() {
            l64
        } else {
            cuda_home.join("lib")
        }
    };

    let supported = supported_arches(&nvcc);
    let gencodes = gencodes(major, &arch, &supported);
    if gencodes.is_empty() {
        println!(
            "cargo:warning=cuda {} cannot target sm_{}, the cpu backend will be used",
            major, arch
        );
        return;
    }

    if out.exists() && std::env::var("QMS_REBUILD_CUDA").is_err() {
        println!(
            "cargo:warning=keeping the prebuilt cuda accelerator at {}",
            out.display()
        );
        println!("cargo:warning=set QMS_REBUILD_CUDA=1 to compile it again");
        return;
    }

    println!("cargo:warning=nvcc {} (cuda {}) arch sm_{}", nvcc.display(), major, arch);
    println!("cargo:warning=building the cuda accelerator into {}", out.display());

    let static_runtime = libdir.join("libcudart_static.a").exists()
        || libdir.join("cudart_static.lib").exists();

    let mut args: Vec<String> = vec![
        "-O3".into(),
        "-shared".into(),
        "-std=c++17".into(),
        "-Wno-deprecated-gpu-targets".into(),
    ];
    if !cfg!(windows) {
        args.push("-Xcompiler".into());
        args.push("-fPIC".into());
    }
    args.push(format!("-I{}", root.join("cuda").join("include").display()));
    args.push(format!("-I{}", root.join("cuda").join("kernels").display()));
    args.push(format!("-L{}", libdir.display()));
    args.extend(gencodes);
    args.push("-o".into());
    args.push(out.to_string_lossy().into_owned());
    args.push(src.to_string_lossy().into_owned());

    if static_runtime {
        args.push("-lcudart_static".into());
        if !cfg!(windows) {
            args.push("-lpthread".into());
            args.push("-ldl".into());
            args.push("-lrt".into());
        }
    } else {
        args.push("-lcudart".into());
    }

    if !run(&nvcc, &args, "compile") {
        let mut retry: Vec<String> = args.clone();
        let at = retry
            .iter()
            .position(|a| a.starts_with("-gencode"))
            .unwrap_or(retry.len());
        retry.retain(|a| {
            !a.starts_with("-gencode")
                && !a.starts_with("arch=")
                && !a.starts_with("code=")
        });
        let native = vec![
            "-gencode".to_string(),
            format!("arch=compute_{},code=sm_{}", arch, arch),
        ];
        for (i, a) in native.into_iter().enumerate() {
            retry.insert(at + i, a);
        }
        if !run(&nvcc, &retry, "retry") {
            println!("cargo:warning=cuda build failed, the cpu backend will be used");
            println!("cargo:warning={}", hint());
        }
    }
}

fn hint() -> &'static str {
    if cfg!(windows) {
        "open a developer command prompt and run nvcc by hand to see the full error"
    } else {
        "run scripts/build_cuda.sh by hand to see the full error"
    }
}

fn run(program: &Path, args: &[String], stage: &str) -> bool {
    let out = match Command::new(program).args(args).output() {
        Ok(v) => v,
        Err(e) => {
            println!("cargo:warning=nvcc could not be started: {}", e);
            return false;
        }
    };
    if out.status.success() {
        return true;
    }
    println!("cargo:warning=nvcc {} failed with {}", stage, out.status);
    let raw = String::from_utf8_lossy(&out.stderr);
    let text = if raw.trim().is_empty() {
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        raw.into_owned()
    };
    let lines: Vec<String> = text
        .lines()
        .map(trim_line)
        .filter(|l| !l.is_empty())
        .collect();
    for line in lines.iter().take(6) {
        println!("cargo:warning={}", line);
    }
    if lines.len() > 6 {
        println!("cargo:warning=and {} more lines", lines.len() - 6);
    }
    false
}

fn trim_line(line: &str) -> String {
    let joined: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() > 110 {
        let cut: String = joined.chars().take(110).collect();
        format!("{} ...", cut)
    } else {
        joined
    }
}

fn pick_nvcc() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("NVCC") {
        if !custom.is_empty() {
            return Some(PathBuf::from(custom));
        }
    }

    let exe = if cfg!(windows) { "nvcc.exe" } else { "nvcc" };
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(exe);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        roots.push(PathBuf::from("C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA"));
    } else {
        roots.push(PathBuf::from("/usr/local/cuda"));
        roots.push(PathBuf::from("/opt/cuda"));
        for base in ["/usr/local", "/opt"] {
            if let Ok(entries) = std::fs::read_dir(base) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with("cuda") {
                        roots.push(entry.path());
                    }
                }
            }
        }
    }

    for base in roots {
        if base.join("bin").join(exe).is_file() {
            return Some(base.join("bin").join(exe));
        }
        if let Ok(entries) = std::fs::read_dir(&base) {
            let mut versions: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("bin").join(exe).is_file())
                .collect();
            versions.sort();
            if let Some(hit) = versions.pop() {
                return Some(hit.join("bin").join(exe));
            }
        }
    }

    None
}

fn nvcc_major(nvcc: &Path) -> u32 {
    let out = Command::new(nvcc).arg("--version").output();
    let Ok(out) = out else { return 0 };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(idx) = line.find("release ") {
            let rest = &line[idx + "release ".len()..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(v) = digits.parse() {
                return v;
            }
        }
    }
    0
}

fn pick_arch() -> String {
    if let Ok(custom) = std::env::var("QMS_CUDA_ARCH") {
        if !custom.is_empty() {
            return custom;
        }
    }
    let smi = if cfg!(windows) { "nvidia-smi.exe" } else { "nvidia-smi" };
    if let Ok(out) = Command::new(smi)
        .args(["--query-gpu=compute_cap", "--format=csv,noheader"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(first) = text.lines().next() {
            let digits: String = first.chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits;
            }
        }
    }
    "75".to_string()
}

fn supported_arches(nvcc: &Path) -> Vec<u32> {
    let Ok(out) = Command::new(nvcc).arg("--list-gpu-arch").output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut out: Vec<u32> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("compute_") {
            if let Ok(n) = rest.parse::<u32>() {
                out.push(n);
            }
        }
    }
    out
}

fn gencode(arch: &str) -> Vec<String> {
    vec![
        "-gencode".into(),
        format!("arch=compute_{},code=sm_{}", arch, arch),
    ]
}

fn ptx(arch: &str) -> Vec<String> {
    vec![
        "-gencode".into(),
        format!("arch=compute_{},code=compute_{}", arch, arch),
    ]
}

fn target_arch(native: &str, supported: &[u32]) -> Option<u32> {
    let n: u32 = native.parse().unwrap_or(75);
    if supported.is_empty() || supported.contains(&n) {
        return Some(n);
    }
    supported.iter().copied().filter(|a| *a >= n).min()
}

fn gencodes(major: u32, native: &str, supported: &[u32]) -> Vec<String> {
    let Some(target) = target_arch(native, supported) else {
        return Vec::new();
    };
    let multi = std::env::var("QMS_CUDA_FAST").is_err();
    let pool: Vec<u32> = if supported.is_empty() {
        vec![50, 52, 53, 60, 61, 62, 70, 72, 75, 80, 86, 87, 89, 90]
    } else {
        supported.to_vec()
    };
    let mut out = Vec::new();
    if multi {
        for a in &pool {
            if *a < 75 && major >= 13 {
                continue;
            }
            out.extend(gencode(&a.to_string()));
        }
    }
    out.extend(gencode(&target.to_string()));
    let mut pt = target;
    for a in pool.iter().copied().chain([100u32, 120, 121]) {
        if a >= target && (supported.is_empty() || supported.contains(&a)) {
            pt = a;
        }
    }
    out.extend(ptx(&pt.to_string()));
    out
}
