
pub const MY_PROJECT: &str = "Qms Faucet";
pub const BASE_URL: &str = "https://faucet.testnet.qms.finance";
pub const RPC_URL: &str = "https://rpc.testnet.qms.finance";
pub const FALLBACK_CHAIN_ID: u64 = 19480;
pub const SOLVER_URL: &str = "";
pub const TURNSTILE_SITEKEY: &str = "0x4AAAAAAFG0vvhuR6nAOoPQ";

pub const TURNSTILE_ACTION: &str = "login";

pub const SOLVE_ATTEMPTS: u32 = 4;
pub const SOLVE_RETRY_SECONDS: u64 = 5;
pub const SOLVE_BUSY_TRIES: u32 = 40;
pub const SOLVE_BUSY_SECONDS: u64 = 3;
pub const SOLVER_POLL_SECONDS: u64 = 2;
pub const SOLVER_POLL_TRIES: u32 = 45;
pub const SHARE_PAUSE_MS: u64 = 120;
pub const MAX_SHARES: u64 = 10000;
pub const CPU_BATCH: u64 = 256;
pub const GPU_BATCH: u64 = 4096;
pub const SELF_TEST_SECONDS: u64 = 25;
pub const SESSION_POLL_SECONDS: u64 = 5;
pub const WS_PING_SECONDS: u64 = 45;
pub const CLAIM_POLL_SECONDS: u64 = 6;
pub const CLAIM_POLL_ATTEMPTS: u32 = 30;
pub const ACCOUNT_PAUSE_SECONDS: u64 = 3;
pub const PROGRESS_SECONDS: u64 = 5;
pub const SESSION_ATTEMPTS: u32 = 3;
pub const SESSION_RETRY_SECONDS: u64 = 15;
pub const CLOSE_GRACE_SECONDS: u64 = 3;
pub const WS_CONNECT_ATTEMPTS: u32 = 3;
pub const WS_CONNECT_RETRY_SECONDS: u64 = 5;
pub const WS_RECONNECT_ATTEMPTS: u32 = 3;
pub const WS_RECONNECT_WAIT_SECONDS: u64 = 5;
pub const SESSION_STATE_PATH: &str = "session.state";
