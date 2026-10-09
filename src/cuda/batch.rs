use crate::cuda::buffer::Staging;
use crate::cuda::ffi::{self, ScanFn};
use crate::cuda::launch::LaunchParams;
use crate::cuda::loader::Library;

pub fn scan(
    lib: &Library,
    preimage: &[u8],
    start_nonce: u64,
    count: u64,
    launch: &LaunchParams,
    mask: &str,
) -> Result<Vec<u64>, i32> {
    let func = lib
        .resolve::<ScanFn>(ffi::SYM_SCAN)
        .ok_or(ffi::QMS_ERR_LIBRARY)?;

    let mut staging = Staging::default();
    let capacity = staging.capacity() as i32;
    let mask_bytes = mask.as_bytes();

    let found = unsafe {
        func(
            preimage.as_ptr(),
            preimage.len() as i32,
            start_nonce,
            count,
            mask_bytes.as_ptr(),
            mask_bytes.len() as i32,
            staging.as_mut_ptr(),
            capacity,
            launch.time_cost,
            launch.mem_cost,
            launch.parallelism,
            launch.version,
            launch.key_length,
        )
    };

    if found < 0 {
        return Err(found);
    }
    if found == 0 {
        return Ok(Vec::new());
    }
    Ok(staging.take(found as usize))
}
