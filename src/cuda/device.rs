use crate::cuda::ffi::{self, DeviceCountFn, DeviceNameFn, DeviceSelectFn, VersionFn};
use crate::cuda::loader::Library;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub index: i32,
    pub name: String,
}

pub fn count(lib: &Library) -> i32 {
    match lib.resolve::<DeviceCountFn>(ffi::SYM_DEVICE_COUNT) {
        Some(f) => unsafe { f() }.max(0),
        None => 0,
    }
}

pub fn name(lib: &Library, index: i32) -> Option<String> {
    let f = lib.resolve::<DeviceNameFn>(ffi::SYM_DEVICE_NAME)?;
    let mut buffer = vec![0i8; 256];
    let written = unsafe { f(index, buffer.as_mut_ptr(), buffer.len() as i32) };
    if written <= 0 {
        return None;
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .take(written as usize)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8(bytes).ok()
}

pub fn select(lib: &Library, index: i32) -> bool {
    match lib.resolve::<DeviceSelectFn>(ffi::SYM_DEVICE_SELECT) {
        Some(f) => {
            let rc = unsafe { f(index) };
            rc == 0
        }
        None => false,
    }
}

pub fn runtime_version(lib: &Library) -> Option<i32> {
    let f = lib.resolve::<VersionFn>(ffi::SYM_VERSION)?;
    Some(unsafe { f() })
}

pub fn list(lib: &Library) -> Vec<DeviceInfo> {
    let total = count(lib);
    let mut out = Vec::new();
    for index in 0..total {
        out.push(DeviceInfo {
            index,
            name: name(lib, index).unwrap_or_else(|| String::from("unnamed device")),
        });
    }
    out
}
