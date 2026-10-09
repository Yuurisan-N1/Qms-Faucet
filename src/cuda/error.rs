use crate::cuda::ffi::{self, ErrorMessageFn};
use crate::cuda::loader::Library;
use crate::support::log::ly;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

pub fn message(lib: &Library, code: i32) -> String {
    let Some(func) = lib.resolve::<ErrorMessageFn>(ffi::SYM_ERROR_MESSAGE) else {
        return format!("cuda error {}", code);
    };
    let ptr = unsafe { func(code) };
    if ptr.is_null() {
        return format!("cuda error {}", code);
    }
    let text = unsafe { std::ffi::CStr::from_ptr(ptr) };
    text.to_string_lossy().into_owned()
}

fn reported() -> &'static Mutex<HashSet<i32>> {
    static SEEN: OnceLock<Mutex<HashSet<i32>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn warn_once(lib: &Library, code: i32) {
    let first = match reported().lock() {
        Ok(mut seen) => seen.insert(code),
        Err(_) => true,
    };
    if first {
        ly(&format!(
            "The gpu accelerator refused a batch with {} ({})",
            code,
            message(lib, code)
        ));
    }
}
