use crate::support::error::{QmsError, QmsResult};
use std::ffi::c_void;
use std::path::PathBuf;

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
#[link(name = "dl")]
extern "C" {
    fn dlopen(path: *const i8, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const i8) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
extern "C" {
    fn dlopen(path: *const i8, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const i8) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
}

#[cfg(windows)]
extern "system" {
    fn LoadLibraryA(path: *const u8) -> *mut c_void;
    fn GetProcAddress(handle: *mut c_void, name: *const u8) -> *mut c_void;
    fn FreeLibrary(handle: *mut c_void) -> i32;
}

pub struct Library {
    handle: *mut c_void,
    path: String,
}

unsafe impl Send for Library {}
unsafe impl Sync for Library {}

pub fn library_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "qms_cuda.dll"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "libqms_cuda.dylib"
    } else {
        "libqms_cuda.so"
    }
}

pub fn candidate_paths() -> Vec<String> {
    let name = library_file_name();
    let mut out = Vec::new();

    if let Ok(custom) = std::env::var("QMS_CUDA_LIB") {
        if !custom.is_empty() {
            out.push(custom);
        }
    }

    out.push(format!("cuda/{}", name));
    out.push(name.to_string());

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("cuda").join(name).to_string_lossy().to_string());
            out.push(dir.join(name).to_string_lossy().to_string());
        }
    }

    out
}

impl Library {
    pub fn load(path: &str) -> QmsResult<Self> {
        let handle = open_library(path);
        if handle.is_null() {
            return Err(QmsError::Cuda(format!(
                "accelerator library {} could not be opened",
                path
            )));
        }
        Ok(Self {
            handle,
            path: path.to_string(),
        })
    }

    pub fn load_any() -> Option<Self> {
        for candidate in candidate_paths() {
            if let Ok(lib) = Self::load(&candidate) {
                return Some(lib);
            }
        }
        None
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reopen(&self) -> Library {
        Library::load(&self.path).unwrap_or_else(|_| Library {
            handle: self.handle,
            path: self.path.clone(),
        })
    }

    pub fn symbol(&self, name: &[u8]) -> Option<*mut c_void> {
        let ptr = lookup_symbol(self.handle, name);
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    pub fn resolve<T: Copy>(&self, name: &[u8]) -> Option<T> {
        self.symbol(name).map(|p| unsafe { crate::cuda::ffi::cast::<T>(p) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            close_library(self.handle);
            self.handle = std::ptr::null_mut();
        }
    }
}

pub fn local_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(not(windows))]
fn open_library(path: &str) -> *mut c_void {
    let mut buffer = Vec::with_capacity(path.len() + 1);
    buffer.extend_from_slice(path.as_bytes());
    buffer.push(0);
    unsafe { dlopen(buffer.as_ptr() as *const i8, 2) }
}

#[cfg(not(windows))]
fn lookup_symbol(handle: *mut c_void, name: &[u8]) -> *mut c_void {
    unsafe { dlsym(handle, name.as_ptr() as *const i8) }
}

#[cfg(not(windows))]
fn close_library(handle: *mut c_void) {
    unsafe {
        dlclose(handle);
    }
}

#[cfg(windows)]
fn open_library(path: &str) -> *mut c_void {
    let mut buffer = Vec::with_capacity(path.len() + 1);
    buffer.extend_from_slice(path.as_bytes());
    buffer.push(0);
    unsafe { LoadLibraryA(buffer.as_ptr()) }
}

#[cfg(windows)]
fn lookup_symbol(handle: *mut c_void, name: &[u8]) -> *mut c_void {
    unsafe { GetProcAddress(handle, name.as_ptr()) }
}

#[cfg(windows)]
fn close_library(handle: *mut c_void) {
    unsafe {
        FreeLibrary(handle);
    }
}
