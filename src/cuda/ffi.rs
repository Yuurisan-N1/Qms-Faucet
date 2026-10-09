use std::ffi::c_void;

pub type DeviceCountFn = unsafe extern "C" fn() -> i32;
pub type DeviceNameFn = unsafe extern "C" fn(i32, *mut i8, i32) -> i32;
pub type DeviceSelectFn = unsafe extern "C" fn(i32) -> i32;
pub type VersionFn = unsafe extern "C" fn() -> i32;
pub type Argon2dFn = unsafe extern "C" fn(
    *const u8,
    i32,
    u64,
    *mut u8,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
) -> i32;
pub type ScanFn = unsafe extern "C" fn(
    *const u8,
    i32,
    u64,
    u64,
    *const u8,
    i32,
    *mut u64,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
) -> i32;

pub type ErrorMessageFn = unsafe extern "C" fn(i32) -> *const i8;

pub const SYM_DEVICE_COUNT: &[u8] = b"qms_cuda_device_count\0";
pub const SYM_DEVICE_NAME: &[u8] = b"qms_cuda_device_name\0";
pub const SYM_DEVICE_SELECT: &[u8] = b"qms_cuda_select\0";
pub const SYM_VERSION: &[u8] = b"qms_cuda_version\0";
pub const SYM_ARGON2D: &[u8] = b"qms_cuda_argon2d\0";
pub const SYM_SCAN: &[u8] = b"qms_cuda_scan\0";
pub const SYM_ERROR_MESSAGE: &[u8] = b"qms_cuda_error_message\0";

pub const QMS_ERR_LIBRARY: i32 = -1;
pub const QMS_ERR_MEMORY: i32 = -3;

pub unsafe fn cast<T: Copy>(ptr: *mut c_void) -> T {
    std::mem::transmute_copy::<*mut c_void, T>(&ptr)
}
