//! Narrow libmpv ABI boundary shared by the native preview and playback probe.
//! Client commands/events run on one worker; render calls run only inside
//! Slint's current-OpenGL rendering notifier. No WebView or child player.
use mpv_sys as ffi;
use std::{
    ffi::{CStr, CString, c_void},
    marker::PhantomData,
    ptr::{self, NonNull},
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy)]
pub enum Layout {
    VideoOnly,
    ProbeChrome,
}
pub struct Client(NonNull<ffi::mpv_handle>, Mutex<Option<Box<RenderWake>>>);
// SAFETY: libmpv's client API is thread-safe. This module serializes event reads
// on the control worker. The UI only uses the independent mpv_render_* API.
unsafe impl Send for Client {}
unsafe impl Sync for Client {}
fn result(code: i32) -> Result<(), String> {
    if code < 0 {
        Err(format!("libmpv error {code}"))
    } else {
        Ok(())
    }
}
impl Client {
    pub(super) fn new(layout: Layout) -> Result<Arc<Self>, String> {
        let (top, bottom) = match layout {
            Layout::VideoOnly => ("0", "0"),
            Layout::ProbeChrome => ("0.10", "0.20"),
        };
        // SAFETY: creates a fresh opaque handle, checked before any dereference.
        let handle =
            NonNull::new(unsafe { ffi::mpv_create() }).ok_or("libmpv allocation failed")?;
        let client = Self(handle, Mutex::new(None));
        for (name, value) in [
            ("config", "no"),
            ("terminal", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("osc", "no"),
            // Sources come only from the owned Rust relay. Container references
            // and auto-discovered files must not open an independent IO path.
            ("access-references", "no"),
            ("autoload-files", "no"),
            ("load-unsafe-playlists", "no"),
            ("demuxer", "+lavf"),
            ("demuxer-lavf-o", "protocol_whitelist=none"),
            ("vo", "libmpv"),
            ("hwdec", "auto-safe"),
            ("idle", "yes"),
            ("pause", "yes"),
            ("keep-open", "yes"),
            ("video-sync", "audio"),
            ("video-timing-offset", "0"),
            ("demuxer-max-bytes", "16MiB"),
            ("demuxer-max-back-bytes", "4MiB"),
            ("network-timeout", "5"),
            ("vd-lavc-threads", "2"),
            ("ad-lavc-threads", "2"),
            ("video-margin-ratio-top", top),
            ("video-margin-ratio-bottom", bottom),
            ("volume", "10"),
        ] {
            let n = CString::new(name).map_err(|e| e.to_string())?;
            let v = CString::new(value).map_err(|e| e.to_string())?;
            // SAFETY: both C strings remain alive through the synchronous call.
            result(unsafe {
                ffi::mpv_set_option_string(client.0.as_ptr(), n.as_ptr(), v.as_ptr())
            })?;
        }
        // SAFETY: fresh configured handle; no render context or concurrent use yet.
        result(unsafe { ffi::mpv_initialize(client.0.as_ptr()) })?;
        Ok(Arc::new(client))
    }
    pub(super) fn command(&self, args: &[&str]) -> Result<(), String> {
        let values = args
            .iter()
            .map(|s| CString::new(*s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut pointers = values.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        pointers.push(ptr::null());
        // SAFETY: null-terminated pointer array and all strings outlive the call.
        result(unsafe { ffi::mpv_command(self.0.as_ptr(), pointers.as_mut_ptr()) })
    }
    pub(super) fn flag(&self, name: &CStr, value: bool) -> Result<(), String> {
        let mut flag = i32::from(value);
        // SAFETY: MPV_FORMAT_FLAG is C int, not Rust bool. libmpv copies it.
        result(unsafe {
            ffi::mpv_set_property(
                self.0.as_ptr(),
                name.as_ptr(),
                ffi::mpv_format_MPV_FORMAT_FLAG,
                (&mut flag as *mut i32).cast(),
            )
        })
    }
    pub(super) fn number(&self, name: &CStr) -> Option<f64> {
        let mut value = 0f64;
        // SAFETY: DOUBLE writes exactly f64 into this initialized aligned slot.
        let code = unsafe {
            ffi::mpv_get_property(
                self.0.as_ptr(),
                name.as_ptr(),
                ffi::mpv_format_MPV_FORMAT_DOUBLE,
                (&mut value as *mut f64).cast(),
            )
        };
        (code >= 0 && value.is_finite()).then_some(value)
    }
    pub(super) fn text(&self, name: &CStr) -> Option<String> {
        // SAFETY: owned property string is valid until mpv_free; never retained.
        let value = unsafe { ffi::mpv_get_property_string(self.0.as_ptr(), name.as_ptr()) };
        if value.is_null() {
            return None;
        }
        // SAFETY: mpv returns a NUL-terminated property string, freed below.
        let text = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .chars()
            .take(128)
            .collect();
        // SAFETY: value came from mpv's property allocator and is freed once.
        unsafe { ffi::mpv_free(value.cast()) };
        Some(text)
    }
    pub(super) fn observe(&self, name: &CStr) -> Result<(), String> {
        // NONE only wakes the worker; typed getters above read current state.
        // SAFETY: name is valid for the synchronous copy, handle remains alive.
        result(unsafe {
            ffi::mpv_observe_property(
                self.0.as_ptr(),
                0,
                name.as_ptr(),
                ffi::mpv_format_MPV_FORMAT_NONE,
            )
        })
    }
    pub(super) fn event(&self) -> Option<(u32, i32)> {
        // SAFETY: only the worker calls wait_event. Read scalar fields before the
        // next call invalidates the event. No event data pointers escape.
        let event = unsafe { ffi::mpv_wait_event(self.0.as_ptr(), 0.) };
        let event = unsafe { event.as_ref() }?;
        let mut error = event.error;
        if event.event_id == ffi::mpv_event_id_MPV_EVENT_END_FILE && !event.data.is_null() {
            // SAFETY: END_FILE data is this exact repr(C) struct until the next
            // wait_event call. Copy only its error scalar; retain no pointer.
            let end = unsafe { &*event.data.cast::<ffi::mpv_event_end_file>() };
            if end.error < 0 {
                error = end.error;
            }
        }
        (event.event_id != ffi::mpv_event_id_MPV_EVENT_NONE).then_some((event.event_id, error))
    }
    pub(super) fn set_wakeup(&self, callback: Box<dyn Fn() + Send + Sync>) -> Result<(), String> {
        let mut slot = self.1.lock().map_err(|_| "event wakeup state poisoned")?;
        if slot.is_some() {
            return Err("event wakeup already installed".into());
        }
        let mut wake = Box::new(RenderWake(callback));
        // SAFETY: stable Box is retained until unregister AND core termination.
        unsafe {
            ffi::mpv_set_wakeup_callback(
                self.0.as_ptr(),
                Some(render_wakeup),
                (&mut *wake as *mut RenderWake).cast(),
            )
        };
        *slot = Some(wake);
        Ok(())
    }
    pub(super) fn get_flag(&self, name: &CStr) -> Option<bool> {
        let mut value = 0i32;
        // SAFETY: FLAG writes a C int into this aligned i32, not a Rust bool.
        let code = unsafe {
            ffi::mpv_get_property(
                self.0.as_ptr(),
                name.as_ptr(),
                ffi::mpv_format_MPV_FORMAT_FLAG,
                (&mut value as *mut i32).cast(),
            )
        };
        (code >= 0).then_some(value != 0)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        // SAFETY: the final Arc outlives the render context and control worker.
        // terminate_destroy joins libmpv's internal work before releasing the core.
        unsafe {
            ffi::mpv_set_wakeup_callback(self.0.as_ptr(), None, ptr::null_mut());
            ffi::mpv_terminate_destroy(self.0.as_ptr())
        }
    }
}

struct Resolver<'a> {
    get: &'a dyn Fn(&CStr) -> *const c_void,
}
unsafe extern "C" fn resolve(data: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void {
    if data.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: mpv 0.42's GL init loads symbols synchronously during create
    // (video/out/opengl/libmpv_gl.c). Resolver and Slint's current loader live
    // through that call; mpv supplies a valid terminated function name.
    let resolver = unsafe { &*data.cast::<Resolver<'_>>() };
    (resolver.get)(unsafe { CStr::from_ptr(name) }).cast_mut()
}
struct RenderWake(Box<dyn Fn() + Send + Sync>);
unsafe extern "C" fn render_wakeup(data: *mut c_void) {
    // SAFETY: stable Box owned by Render, unregistered/freed only after the
    // context is destroyed. Its closure cannot panic and only coalesces wakeups.
    if let Some(wake) = unsafe { data.cast::<RenderWake>().as_ref() } {
        (wake.0)();
    }
}
pub struct Render {
    core: Arc<Client>,
    context: NonNull<ffi::mpv_render_context>,
    wake: Box<RenderWake>,
    _thread: PhantomData<Rc<()>>,
}
impl Render {
    /// Called only in RenderingSetup with Slint's current GL context. Drop only
    /// in RenderingTeardown/BeforeRendering on that same context and thread.
    /// # Safety
    /// The supplied GL context must be current on this thread during create,
    /// every draw, and destruction. The core must outlive this render context.
    pub unsafe fn new(
        core: Arc<Client>,
        get: &dyn Fn(&CStr) -> *const c_void,
        wake: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let mut resolver = Resolver { get };
        let mut init = ffi::mpv_opengl_init_params {
            get_proc_address: Some(resolve),
            get_proc_address_ctx: (&mut resolver as *mut Resolver<'_>).cast(),
        };
        let mut params = [
            ffi::mpv_render_param {
                type_: 1,
                data: c"opengl".as_ptr().cast_mut().cast(),
            },
            ffi::mpv_render_param {
                type_: 2,
                data: (&mut init as *mut ffi::mpv_opengl_init_params).cast(),
            },
            ffi::mpv_render_param {
                type_: 0,
                data: ptr::null_mut(),
            },
        ];
        let mut context = ptr::null_mut();
        // SAFETY: exact repr(C) bindings; sentinel-terminated stack params and
        // loader remain valid for synchronous create. Core is retained by Arc.
        result(unsafe {
            ffi::mpv_render_context_create(&mut context, core.0.as_ptr(), params.as_mut_ptr())
        })?;
        let context = NonNull::new(context).ok_or("libmpv returned null render context")?;
        let mut owner = Self {
            core,
            context,
            wake: Box::new(RenderWake(wake)),
            _thread: PhantomData,
        };
        // SAFETY: Box allocation is stable, callback has no client/render calls.
        unsafe {
            ffi::mpv_render_context_set_update_callback(
                owner.context.as_ptr(),
                Some(render_wakeup),
                (&mut *owner.wake as *mut RenderWake).cast(),
            )
        };
        Ok(owner)
    }
    /// # Safety
    /// Same current GL context/thread as creation; fbo must be complete, live,
    /// and owned by the caller for the supplied dimensions throughout the call.
    pub unsafe fn draw(&self, fbo: i32, width: i32, height: i32) -> Result<(), String> {
        if width <= 0
            || height <= 0
            || width > 8192
            || height > 8192
            || i64::from(width) * i64::from(height) > 16_777_216
        {
            return Err("video surface exceeds pixel budget".into());
        }
        // Includes internal_format: omitting this fourth field is an ABI error.
        let mut target = ffi::mpv_opengl_fbo {
            fbo,
            w: width,
            h: height,
            internal_format: 0,
        };
        let mut flip = 1i32;
        let mut block = 0i32;
        let mut params = [
            ffi::mpv_render_param {
                type_: 3,
                data: (&mut target as *mut ffi::mpv_opengl_fbo).cast(),
            },
            ffi::mpv_render_param {
                type_: 4,
                data: (&mut flip as *mut i32).cast(),
            },
            ffi::mpv_render_param {
                type_: 12,
                data: (&mut block as *mut i32).cast(),
            },
            ffi::mpv_render_param {
                type_: 0,
                data: ptr::null_mut(),
            },
        ];
        // SAFETY: same GL context/thread as create; stack data survives render.
        // No target-time blocking and no advanced-control wait on the UI thread.
        unsafe {
            ffi::mpv_render_context_update(self.context.as_ptr());
        }
        result(unsafe {
            ffi::mpv_render_context_render(self.context.as_ptr(), params.as_mut_ptr())
        })
    }
}
impl Drop for Render {
    fn drop(&mut self) {
        // SAFETY: notifier teardown runs on the same current GL context. Context
        // destruction synchronizes libmpv work before callback Box/Core can drop.
        unsafe {
            ffi::mpv_render_context_set_update_callback(
                self.context.as_ptr(),
                None,
                ptr::null_mut(),
            );
            ffi::mpv_render_context_free(self.context.as_ptr());
        }
        let _ = &self.core;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_core_roundtrips_c_integer_flags_and_releases() {
        let core = Client::new(Layout::VideoOnly).unwrap();
        let lifetime = Arc::downgrade(&core);
        assert_eq!(core.get_flag(c"pause"), Some(true));
        core.flag(c"pause", false).unwrap();
        assert_eq!(core.get_flag(c"pause"), Some(false));
        core.flag(c"pause", true).unwrap();
        assert_eq!(core.get_flag(c"pause"), Some(true));
        assert_eq!(core.get_flag(c"pttstella-unknown-property"), None);
        drop(core);
        assert!(lifetime.upgrade().is_none());
    }
}
