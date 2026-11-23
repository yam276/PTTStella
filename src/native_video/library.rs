//! libmpv entry points. macOS and Windows packages ship libmpv inside the App
//! and link it directly. Linux uses the distribution's libmpv2 and loads it on
//! first use, so the application still starts without that package and only
//! native video is unavailable.
use mpv_sys as ffi;
use std::{
    ffi::{c_char, c_int, c_void},
    sync::OnceLock,
};

type Handle = *mut ffi::mpv_handle;
type Context = *mut ffi::mpv_render_context;
type Callback = Option<unsafe extern "C" fn(*mut c_void)>;

/// The libmpv functions this crate calls, typed exactly as libmpv2-sys 4.0.1.
pub(super) struct Api {
    pub create: unsafe extern "C" fn() -> Handle,
    pub initialize: unsafe extern "C" fn(Handle) -> c_int,
    pub set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    pub set_property:
        unsafe extern "C" fn(Handle, *const c_char, ffi::mpv_format, *mut c_void) -> c_int,
    pub get_property:
        unsafe extern "C" fn(Handle, *const c_char, ffi::mpv_format, *mut c_void) -> c_int,
    pub get_property_string: unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char,
    pub free: unsafe extern "C" fn(*mut c_void),
    pub observe_property:
        unsafe extern "C" fn(Handle, u64, *const c_char, ffi::mpv_format) -> c_int,
    pub wait_event: unsafe extern "C" fn(Handle, f64) -> *mut ffi::mpv_event,
    pub set_wakeup_callback: unsafe extern "C" fn(Handle, Callback, *mut c_void),
    pub terminate_destroy: unsafe extern "C" fn(Handle),
    pub render_context_create:
        unsafe extern "C" fn(*mut Context, Handle, *mut ffi::mpv_render_param) -> c_int,
    pub render_context_set_update_callback:
        unsafe extern "C" fn(Context, ffi::mpv_render_update_fn, *mut c_void),
    pub render_context_update: unsafe extern "C" fn(Context) -> u64,
    pub render_context_get_info: unsafe extern "C" fn(Context, ffi::mpv_render_param) -> c_int,
    pub render_context_render: unsafe extern "C" fn(Context, *mut ffi::mpv_render_param) -> c_int,
    pub render_context_free: unsafe extern "C" fn(Context),
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

pub(super) fn api() -> Result<&'static Api, String> {
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

/// Whether native video can start. On Linux the first call loads libmpv2;
/// later calls reuse that result for the life of the process.
pub fn available() -> Result<(), String> {
    api().map(|_| ())
}

#[cfg(not(target_os = "linux"))]
fn load() -> Result<Api, String> {
    Ok(Api {
        create: ffi::mpv_create,
        initialize: ffi::mpv_initialize,
        set_option_string: ffi::mpv_set_option_string,
        command: ffi::mpv_command,
        set_property: ffi::mpv_set_property,
        get_property: ffi::mpv_get_property,
        get_property_string: ffi::mpv_get_property_string,
        free: ffi::mpv_free,
        observe_property: ffi::mpv_observe_property,
        wait_event: ffi::mpv_wait_event,
        set_wakeup_callback: ffi::mpv_set_wakeup_callback,
        terminate_destroy: ffi::mpv_terminate_destroy,
        render_context_create: ffi::mpv_render_context_create,
        render_context_set_update_callback: ffi::mpv_render_context_set_update_callback,
        render_context_update: ffi::mpv_render_context_update,
        render_context_get_info: ffi::mpv_render_context_get_info,
        render_context_render: ffi::mpv_render_context_render,
        render_context_free: ffi::mpv_render_context_free,
    })
}

#[cfg(target_os = "linux")]
fn load() -> Result<Api, String> {
    // The Ubuntu/Debian libmpv2 package's runtime name.
    open("libmpv.so.2".as_ref())
}

#[cfg(any(target_os = "linux", test))]
fn open(name: &std::ffi::OsStr) -> Result<Api, String> {
    // SAFETY: loading a shared library runs its initializers. Only the named
    // system player library is opened, never a path taken from content.
    let library = unsafe { libloading::Library::new(name) }.map_err(|e| e.to_string())?;
    macro_rules! symbol {
        ($name:literal) => {
            // SAFETY: each field has the exact libmpv2-sys binding type for this
            // symbol, and the library stays mapped for the process (below).
            *unsafe { library.get($name) }.map_err(|e| e.to_string())?
        };
    }
    let api = Api {
        create: symbol!(b"mpv_create\0"),
        initialize: symbol!(b"mpv_initialize\0"),
        set_option_string: symbol!(b"mpv_set_option_string\0"),
        command: symbol!(b"mpv_command\0"),
        set_property: symbol!(b"mpv_set_property\0"),
        get_property: symbol!(b"mpv_get_property\0"),
        get_property_string: symbol!(b"mpv_get_property_string\0"),
        free: symbol!(b"mpv_free\0"),
        observe_property: symbol!(b"mpv_observe_property\0"),
        wait_event: symbol!(b"mpv_wait_event\0"),
        set_wakeup_callback: symbol!(b"mpv_set_wakeup_callback\0"),
        terminate_destroy: symbol!(b"mpv_terminate_destroy\0"),
        render_context_create: symbol!(b"mpv_render_context_create\0"),
        render_context_set_update_callback: symbol!(b"mpv_render_context_set_update_callback\0"),
        render_context_update: symbol!(b"mpv_render_context_update\0"),
        render_context_get_info: symbol!(b"mpv_render_context_get_info\0"),
        render_context_render: symbol!(b"mpv_render_context_render\0"),
        render_context_free: symbol!(b"mpv_render_context_free\0"),
    };
    // The function pointers are only valid while the library is mapped, and
    // the loaded table lives in a process-wide static: never unload it.
    std::mem::forget(library);
    Ok(api)
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_library_is_reported_instead_of_stopping_startup() {
        let error = super::open("libpttstella-missing-player.so.0".as_ref())
            .err()
            .expect("a missing library must not load");
        assert!(!error.is_empty());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn runtime_loading_resolves_every_entry_point_from_the_built_libmpv() {
        let name = if cfg!(target_os = "macos") {
            "libmpv.2.dylib"
        } else {
            "libmpv.so.2"
        };
        let path = std::path::Path::new(env!("PTTSTELLA_MPV_LIB_DIR")).join(name);
        let api = super::open(path.as_os_str()).unwrap();
        // SAFETY: one fresh handle is initialized and destroyed through the
        // same loaded table; nothing else uses it.
        unsafe {
            let handle = (api.create)();
            assert!(!handle.is_null());
            assert!((api.initialize)(handle) >= 0);
            (api.terminate_destroy)(handle);
        }
    }
}
