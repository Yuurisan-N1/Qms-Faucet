use crate::cuda::ffi::{self, Argon2dFn};
use crate::cuda::launch::LaunchParams;
use crate::cuda::loader::Library;

pub fn argon2d(
    lib: &Library,
    preimage: &[u8],
    nonce: u64,
    launch: &LaunchParams,
) -> Result<Vec<u8>, i32> {
    let func = lib
        .resolve::<Argon2dFn>(ffi::SYM_ARGON2D)
        .ok_or(ffi::QMS_ERR_LIBRARY)?;

    let mut out = vec![0u8; 64];
    let len = launch.key_length.clamp(1, 64);

    let rc = unsafe {
        func(
            preimage.as_ptr(),
            preimage.len() as i32,
            nonce,
            out.as_mut_ptr(),
            len,
            launch.time_cost,
            launch.mem_cost,
            launch.parallelism,
            launch.version,
            launch.key_length,
        )
    };

    if rc < 0 {
        return Err(rc);
    }

    out.truncate(len as usize);
    Ok(out)
}
