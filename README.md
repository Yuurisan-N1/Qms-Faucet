<div align="center">

<img width="100%" alt="header" src="https://capsule-render.vercel.app/api?type=waving&height=210&text=QMS%20Faucet%20Bot&fontAlign=50&fontAlignY=36&fontSize=56&desc=Argon2d%20PoW%20Mining%20%7C%20GPU%20Accelerated%20%7C%20Auto%20Claim%20%7C%20Multi-Address&descAlign=50&descAlignY=58"/>

<img alt="typing" src="https://readme-typing-svg.demolab.com?font=Inter&size=18&duration=3000&pause=650&center=true&vCenter=true&width=900&lines=Argon2d+Proof-of-Work+Mining+%7C+CPU+%26+GPU;CUDA+GPU+Backend+%7C+sm_75+%2F+sm_86+%2F+sm_89+%2F+sm_90;Auto+Claim+Faucet+%7C+After+Each+Valid+Solution;6+Run+Modes+%7C+loop+%2F+mine+%2F+claim+%2F+probe+%2F+once+%2F+selftest;Multi-Address+%7C+All+Wallets+Per+Cycle"/>

<p>
  <img alt="rust" src="https://img.shields.io/badge/Rust-2021-f74c00?logo=rust&logoColor=white"/>
  <img alt="platform" src="https://img.shields.io/badge/Platform-QMS%20Testnet-111111"/>
  <img alt="multi-address" src="https://img.shields.io/badge/Multi--Address-Supported-111111"/>
  <img alt="author" src="https://img.shields.io/badge/by-Yuurisandesu-111111"/>
</p>

<p>
  <b>QMS Faucet Bot</b> is an Argon2d proof-of-work mining bot for the QMS Testnet faucet.<br/>
  It fetches the current mining challenge from the faucet server, solves the Argon2d proof-of-work puzzle to the configured difficulty target on CPU or GPU, submits the solution to claim the faucet reward, and repeats for each wallet address in the list, with six run modes, CUDA GPU acceleration across four compute capability tiers, proxy support, and a loop cycle with configurable sleep.<br/>
  Built and distributed by <b>Yuurisandesu</b>.
</p>

</div>

---

## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Configuration](#configuration)
- [Running the Bot](#running-the-bot)
- [Run Modes](#run-modes)
- [GPU Backend](#gpu-backend)
- [Download Prebuilt Binary](#download-prebuilt-binary)
- [Features](#features)
- [File Structure](#file-structure)
- [Disclaimer](#disclaimer)

---

## Requirements

- Rust `1.70+` (includes `cargo`) -- only needed if building from source
- Ruby `3.0+` -- only needed to run the downloader script
- Git
- CUDA Toolkit `12.x` -- optional, only needed to build the GPU backend

---

## Installation

### Install Rust

**Linux / macOS:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Windows:**

Download and run the installer from https://rustup.rs, then restart your terminal.

**Termux (Android):**

```bash
pkg update && pkg install proot-distro
proot-distro install ubuntu
proot-distro login ubuntu
```

Then inside Ubuntu:

```bash
apt update && apt install -y curl git build-essential pkg-config libssl-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Install Ruby (for downloader only)

**Linux:**

```bash
sudo apt install ruby
```

**macOS:**

```bash
brew install ruby
```

**Windows:** Download from https://rubyinstaller.org and follow the installer.

**Termux:**

```bash
pkg install ruby
```

### Clone the Repository

```bash
git clone https://github.com/Yuurisan-N1/Qms-Faucet.git
cd Qms-Faucet
```

---

## Configuration

### 1. Wallet Addresses (address.txt)

Fill `address.txt` with EVM wallet addresses to claim faucet rewards for, one per line:

```
0xYourWalletAddressHere
0xAnotherWalletAddressHere
```

### 2. Proxy (proxy.txt)

Fill `proxy.txt` with proxies, one per line (optional, leave empty to run without proxy):

```
host:port
host:port:user:pass
http://user:pass@host:port
socks5://user:pass@host:port
```

Proxies are assigned to addresses by index in round-robin order.

### 3. Bot Settings (config.json)

| Setting | Description |
|---|---|
| `settings.sleep_seconds` | Seconds to wait between full cycles in loop mode |
| `settings.workers` | Number of CPU threads for mining. Set to `0` to use all available cores |
| `settings.target_qms` | Target QMS amount to mine per address before stopping. Set to `0` to mine indefinitely |

---

## Running the Bot

### Using run.sh (Linux / macOS / Termux)

Make it executable first:

```bash
chmod +x run.sh
```

Then choose a mode:

```bash
./run.sh direct    # build and run in foreground (default)
./run.sh nohup     # build and run in background, logs saved to qms-faucet-bot.log
./run.sh screen    # build and run in a detached screen session
./run.sh tmux      # build and run in a detached tmux session
./run.sh logs      # tail the log file
./run.sh stop      # stop the running bot process
```

Attach to a background session anytime:

```bash
# screen
screen -r qms-faucet-bot

# tmux
tmux attach -t qms-faucet-bot
```

### Using make (Linux / macOS)

```bash
make release    # optimized release build
make start      # build release and run immediately
make run        # build debug and run
make check      # check for errors without building
make fmt        # format the code
make clean      # remove all build artifacts
make cuda       # build the optional GPU accelerator (requires CUDA Toolkit)
make selftest   # run built-in Argon2d self-test
make size       # show release binary size
make package    # build release and package for distribution
```

### Manual cargo build

```bash
cargo build --release
```

Then run with a mode argument:

```bash
# Linux / macOS / Termux
./target/release/qms-faucet-bot loop

# Windows
.\target\release\qms-faucet-bot.exe loop
```

Omit the mode argument to default to `loop`.

---

## Run Modes

The binary accepts a mode argument to control its behavior:

| Mode | Description |
|---|---|
| `loop` | Default mode. Mine and claim for all addresses in a loop, sleeping `sleep_seconds` between cycles |
| `once` | Mine and claim for all addresses once, then exit |
| `mine` | Mine only, do not submit any claim. Useful for benchmarking or pre-solving |
| `claim` | Skip mining, submit a claim attempt directly. Useful if a solution was already computed |
| `probe` | Probe the server to fetch and display the current challenge parameters without mining or claiming |
| `selftest` | Run the built-in Argon2d test vectors from `assets/argon2d_vectors.json` to verify the implementation is correct |

Example:

```bash
./target/release/qms-faucet-bot probe
./target/release/qms-faucet-bot selftest
./target/release/qms-faucet-bot once
```

---

## GPU Backend

To enable GPU acceleration for Argon2d mining, build the CUDA shared library using the provided script. CUDA Toolkit `12.x` and `nvcc` are required. The script auto-detects the installed GPU compute capability and compiles for the appropriate architecture:

| Compute Capability | GPU Generation |
|---|---|
| sm_75 | Turing (RTX 20xx, GTX 16xx) |
| sm_86 | Ampere (RTX 30xx) |
| sm_89 | Ada Lovelace (RTX 40xx) |
| sm_90 | Hopper (H100) |

Build:

```bash
bash scripts/build_cuda.sh
# or
make cuda
```

The compiled library is placed in the `cuda/` folder. When the bot starts, it detects and loads the library automatically. If the library is absent or the GPU is not supported, the bot falls back to the CPU thread pool without any configuration change required.

---

## Download Prebuilt Binary

### Option 1 - Downloader Script

```bash
ruby downloader/bot.rb
```

The script shows a numbered menu:

```
1. Qms Faucet Windows x86_64
2. Qms Faucet Linux x86_64
3. Qms Faucet macOS ARM64
```

Enter the number for your platform. The binary downloads with a live progress bar and is set to executable automatically on Linux and macOS.

Place the binary in the same folder as your `address.txt`, `proxy.txt`, and `config.json` before running.

### Option 2 - Manual Download

Download the latest release from:
https://github.com/Yuurisan-N1/Qms-Faucet/releases/latest

| File | Platform |
|---|---|
| `Qms-Faucet-Windows-x86_64.exe` | Windows x86_64 |
| `qms-faucet-bot-linux-x86_64` | Linux x86_64 |
| `qms-faucet-bot-macos-aarch64` | macOS Apple Silicon |

**Linux / macOS after downloading:**

```bash
chmod +x qms-faucet-bot-linux-x86_64
./qms-faucet-bot-linux-x86_64 loop
```

**Windows:**

```bash
.\Qms-Faucet-Windows-x86_64.exe loop
```

---

## Features

### Argon2d Proof-of-Work Mining
The bot fetches the current mining challenge from the faucet server including the nonce, difficulty target, and Argon2d parameters. It then computes Argon2d hashes across the CPU thread pool or the GPU backend until a hash meets the difficulty target. The number of CPU workers is configurable and defaults to all available cores.

### CUDA GPU Acceleration
When the CUDA shared library is present, the bot offloads Argon2d hash computation to the GPU. The library supports four GPU compute capability tiers: sm_75, sm_86, sm_89, and sm_90. The GPU backend is detected and loaded at startup. If unavailable, the CPU pool is used automatically.

### Auto Claim
After a valid proof-of-work solution is found, the bot submits it to the faucet server along with the wallet address and claims the QMS reward. The claimed amount is logged on success.

### Target QMS
Set `target_qms` in config to stop after a wallet has accumulated the configured amount. Set it to `0` to mine indefinitely per address.

### Multi Address
All addresses in `address.txt` are processed sequentially within every cycle. Each address gets its own proxy from the pool.

### Six Run Modes
`loop`, `once`, `mine`, `claim`, `probe`, and `selftest` cover every use case from continuous automation to benchmarking and debugging. See [Run Modes](#run-modes) above.

### Graceful Shutdown
Pressing `Ctrl+C` once signals all miners to finish their current batch and exit cleanly. Pressing it twice forces an immediate exit.

### Proxy Support
Proxies are loaded from `proxy.txt` and assigned to addresses by position in round-robin order. HTTP and SOCKS5 proxies with or without credentials are supported. Running without proxies is fully supported.

---

## File Structure

```text
Qms-Faucet/
├── src/
│   ├── main.rs                          # Entry point, mode dispatch, shutdown handler
│   ├── cuda/                            # Rust FFI bindings to the CUDA shared library
│   ├── domain/                          # Account, faucet challenge, mining params models
│   ├── mine/
│   │   ├── argon2d.rs                   # Argon2d hasher core
│   │   ├── backend.rs                   # Backend selector: CPU or GPU
│   │   ├── cpu.rs                       # CPU thread pool worker
│   │   ├── gpu.rs                       # GPU dispatch via CUDA FFI
│   │   ├── hw.rs                        # Hardware info: CPU cores, GPU detection
│   │   └── worker.rs                    # Mining loop: fetch, solve, submit
│   ├── modes/
│   │   ├── claim_only.rs                # Claim mode: skip mining, submit directly
│   │   ├── mine_only.rs                 # Mine mode: solve only, no claim
│   │   ├── probe.rs                     # Probe mode: fetch and display challenge
│   │   ├── run_loop.rs                  # Loop mode: mine and claim in a cycle
│   │   ├── run_once.rs                  # Once mode: single pass all addresses
│   │   └── selftest.rs                  # Selftest mode: run Argon2d test vectors
│   ├── net/                             # HTTP client, proxy pool, retry logic, RPC, session
│   ├── ops/
│   │   ├── bootstrap.rs                 # Session and challenge bootstrap
│   │   ├── captcha.rs                   # Captcha handling
│   │   ├── claim.rs                     # Faucet claim submission
│   │   ├── mining.rs                    # Mining orchestration
│   │   └── pipeline.rs                  # Full per-address pipeline
│   └── support/                         # Banner, config loader, clock, logger, shutdown, consts
├── cuda/
│   ├── arch/                            # Architecture-specific CUDA definitions (sm_75 to sm_90)
│   ├── include/                         # CUDA headers and type definitions
│   └── kernels/
│       ├── argon2_blake2b.cuh           # Blake2b used in Argon2d initialization
│       ├── argon2_kernel.cu             # Main Argon2d CUDA kernel
│       └── common.cuh                   # Shared CUDA utilities
├── assets/
│   ├── argon2d_vectors.json             # Argon2d test vectors for selftest mode
│   └── icon.ico                         # Windows binary icon
├── scripts/
│   ├── build_cuda.sh                    # Build script for CUDA shared library
│   └── package.sh                       # Package script for release distribution
├── downloader/
│   ├── bot.rb                           # Interactive downloader script (Ruby)
│   └── utils/
│       └── banner.rb                    # Banner display on startup
├── Cargo.toml                           # Project manifest and dependencies
├── Cargo.lock
├── build.rs                             # Build script (embeds icon into Windows binary)
├── Makefile                             # make targets: build, run, release, cuda, selftest, package
├── run.sh                               # Run helper: direct, nohup, screen, tmux, logs, stop
├── config.json                          # Workers, sleep, target QMS settings
├── address.txt                          # Wallet addresses, one per line
└── proxy.txt                            # Proxy list (optional)
```

---

## Disclaimer

This tool is built for educational and technical exploration purposes. Use it wisely and at your own responsibility.

---

<div align="center">
<img width="100%" alt="footer" src="https://capsule-render.vercel.app/api?type=waving&height=120&section=footer"/>
</div>