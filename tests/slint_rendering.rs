//! Compare independent full/retained Skia surfaces using the production view.
//! This is an offscreen dirty-region audit, not OS/native presentation evidence.
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_renderer_skia::{
    SkiaRenderer, SkiaSharedContext,
    software_surface::{RenderBuffer, SoftwareSurface},
};
use slint::{
    ComponentHandle, Image, Model, ModelRc, PhysicalSize, Rgba8Pixel, SharedPixelBuffer, VecModel,
    Window, WindowSize,
    platform::{Platform, PlatformError, Renderer, WindowAdapter, WindowEvent},
};
use std::{
    cell::{Cell, RefCell},
    num::NonZeroU32,
    rc::Rc,
    time::Duration,
};

slint::include_modules!();

#[derive(Default)]
struct Raster {
    size: (u32, u32),
    pixels: Vec<u8>,
    valid: bool,
}

struct MemoryBuffer {
    retained: bool,
    raster: RefCell<Raster>,
    last_age: Cell<u8>,
    last_area: Cell<Option<f32>>,
    frames: Cell<usize>,
}

impl RenderBuffer for MemoryBuffer {
    fn with_buffer(
        &self,
        _window: &Window,
        size: PhysicalSize,
        callback: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<Option<DirtyRegion>, PlatformError>,
    ) -> Result<(), PlatformError> {
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Err("zero-sized audit surface".into());
        };
        let mut raster = self.raster.borrow_mut();
        if raster.size != (size.width, size.height) || !self.retained {
            raster.size = (size.width, size.height);
            raster.pixels = vec![0; size.width as usize * size.height as usize * 4];
            raster.valid = false;
        }
        let age = u8::from(raster.valid && self.retained);
        raster.valid = false;
        let dirty = callback(
            width,
            height,
            skia_safe::ColorType::BGRA8888,
            age,
            &mut raster.pixels,
        )?;
        self.last_age.set(age);
        self.last_area
            .set(dirty.as_ref().map(|r| r.iter().map(|r| r.area()).sum()));
        self.frames.set(self.frames.get() + 1);
        raster.valid = true;
        Ok(())
    }
}

struct AuditWindow {
    window: Window,
    renderer: SkiaRenderer,
    size: Cell<PhysicalSize>,
    buffer: Rc<MemoryBuffer>,
}

impl AuditWindow {
    fn new(retained: bool) -> Rc<Self> {
        let buffer = Rc::new(MemoryBuffer {
            retained,
            raster: RefCell::default(),
            last_age: Cell::new(0),
            last_area: Cell::new(None),
            frames: Cell::new(0),
        });
        Rc::<Self>::new_cyclic(|weak| Self {
            window: Window::new(weak.clone()),
            renderer: SkiaRenderer::new_with_surface(
                &SkiaSharedContext::default(),
                Box::new(SoftwareSurface::from(buffer.clone())),
            ),
            size: Cell::new(PhysicalSize::new(1040, 740)),
            buffer,
        })
    }

    fn resize(&self, size: PhysicalSize) {
        self.size.set(size);
        self.window.dispatch_event(WindowEvent::Resized {
            size: size.to_logical(self.window.scale_factor()),
        });
    }
}

impl WindowAdapter for AuditWindow {
    fn window(&self) -> &Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: WindowSize) {
        self.resize(size.to_physical(self.window.scale_factor()));
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
}

struct AuditPlatform {
    windows: Rc<RefCell<Vec<Rc<AuditWindow>>>>,
}

impl Platform for AuditPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let window = AuditWindow::new(!self.windows.borrow().is_empty());
        self.windows.borrow_mut().push(window.clone());
        Ok(window)
    }

    fn duration_since_start(&self) -> Duration {
        Duration::ZERO
    }
}

fn solid(width: u32, height: u32, colour: [u8; 4]) -> Image {
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    for pixel in pixels.make_mut_slice() {
        *pixel = Rgba8Pixel::new(colour[0], colour[1], colour[2], colour[3]);
    }
    Image::from_rgba8(pixels)
}

fn picture(key: i32, y: f32, blue: bool) -> InlinePicture {
    InlinePicture {
        index: key,
        key,
        stamp: "synthetic-render-audit".into(),
        x: 14.,
        y,
        width: 512.,
        height: 160.,
        image_y: 0.,
        image_height: 160.,
        source_y: 0,
        source_width: 512,
        source_height: 160,
        image: solid(
            512,
            160,
            if blue {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 255]
            },
        ),
        status: "".into(),
    }
}

fn tiles(view: &TerminalWindow, gaps: [f32; 2]) {
    let mut y = 0.;
    let mut rows = Vec::new();
    for row in 0..23 {
        rows.push(ArticleTile {
            y,
            height: 24.,
            image: solid(1000, 24, [42 + row, 80, 100, 255]),
        });
        y += 24.;
        if row == 1 {
            y += gaps[0];
        } else if row == 3 {
            y += gaps[1];
        }
    }
    // A fresh model also retires the previous repeated item trees, as the
    // production article projection does when image metadata changes flow.
    view.set_article_tiles(ModelRc::new(VecModel::from(rows)));
    view.set_article_document_height(y);
}

fn loading(key: i32, y: f32) -> InlinePicture {
    InlinePicture {
        width: 440.,
        height: 36.,
        image_height: 36.,
        source_width: 1,
        source_height: 1,
        image: Image::default(),
        status: "Loading synthetic image…".into(),
        ..picture(key, y, false)
    }
}

fn compare(windows: &[Rc<AuditWindow>], label: &str) {
    slint::platform::update_timers_and_animations();
    for window in windows {
        assert!(matches!(
            window.renderer.render().unwrap(),
            i_slint_core::renderer::DrawOutcome::Success
        ));
    }
    let full = windows[0].buffer.raster.borrow();
    let partial = windows[1].buffer.raster.borrow();
    assert_eq!(full.size, partial.size, "{label}");
    let differences: Vec<_> = full
        .pixels
        .chunks_exact(4)
        .zip(partial.pixels.chunks_exact(4))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .take(12)
        .map(|(index, (a, b))| {
            (
                index % full.size.0 as usize,
                index / full.size.0 as usize,
                a,
                b,
            )
        })
        .collect();
    assert!(
        differences.is_empty(),
        "{label}: first unequal BGRA pixels {differences:?}"
    );
}

fn check_colour(window: &AuditWindow, bgra: [u8; 4], label: &str) {
    assert!(
        window
            .buffer
            .raster
            .borrow()
            .pixels
            .chunks_exact(4)
            .filter(|p| *p == bgra)
            .count()
            >= 1000,
        "missing independent synthetic colour {bgra:?}: {label}"
    );
}

#[test]
fn full_and_partial_terminal_views_match_during_inline_and_layout_changes() {
    let windows = Rc::new(RefCell::new(Vec::new()));
    slint::platform::set_platform(Box::new(AuditPlatform {
        windows: windows.clone(),
    }))
    .unwrap();
    let views = [
        TerminalWindow::new().unwrap(),
        TerminalWindow::new().unwrap(),
    ];
    let models = [Rc::new(VecModel::default()), Rc::new(VecModel::default())];
    for (view, model) in views.iter().zip(&models) {
        view.set_prototype_chrome(false);
        view.set_session_active(true);
        view.set_terminal_input(true);
        view.set_article_active(false);
        view.set_terminal_image(solid(1000, 720, [10, 90, 30, 255]));
        view.set_raster_width(1000.);
        view.set_raster_height(720.);
        view.set_inline_enabled(true);
        view.set_inline_pictures(ModelRc::from(model.clone()));
        view.set_article_line_height(24.);
        view.set_article_line_width(1000.);
        view.set_article_body_height(650.);
        view.set_article_footer(solid(1000, 24, [190, 190, 190, 255]));
        tiles(view, [0., 0.]);
        view.show().unwrap();
    }
    let windows = windows.borrow();
    assert_eq!(windows.len(), 2);
    for window in windows.iter() {
        window.resize(PhysicalSize::new(1040, 740));
    }
    compare(&windows, "initial terminal");
    check_colour(&windows[0], [30, 90, 10, 255], "initial terminal");
    for view in &views {
        view.set_article_active(true);
    }
    let check = |label: &str| {
        compare(&windows, label);
        check_colour(&windows[0], [190, 190, 190, 255], label);
    };
    check("terminal to article");
    for (window, view) in windows.iter().zip(&views) {
        window
            .window
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        view.invoke_focus_terminal();
    }
    check("article focus");
    let geometry = |view: &TerminalWindow| {
        (
            view.get_terminal_origin_x(),
            view.get_terminal_origin_y(),
            view.get_view_width(),
            view.get_view_height(),
        )
    };
    let before_filters = views.each_ref().map(geometry);
    for (_, _, width, height) in before_filters {
        assert_eq!(
            (width, height),
            (1040., 740.),
            "article viewport must fill the window"
        );
    }
    for count in [1, 17, 0] {
        for view in &views {
            view.set_filter_count(count);
            view.set_filter_status(format!("Masked synthetic rows: {count}").into());
            view.set_filter_label("Temporarily reveal hidden rows".into());
            view.set_filter_ready(true);
        }
        check("filter count changed");
        assert_eq!(
            views.each_ref().map(geometry),
            before_filters,
            "filter feedback moved/resized the acknowledged article viewport"
        );
    }
    for (model, view) in models.iter().zip(&views) {
        model.push(loading(0, 54.));
        tiles(view, [44., 0.]);
    }
    check("first image loading");
    for (model, view) in models.iter().zip(&views) {
        model.set_row_data(0, picture(0, 54., false));
        tiles(view, [168., 0.]);
    }
    check("first image metadata");
    for (model, view) in models.iter().zip(&views) {
        model.push(loading(1, 272.));
        tiles(view, [168., 44.]);
    }
    check("second image loading");
    for (model, view) in models.iter().zip(&views) {
        model.set_row_data(1, picture(1, 272., true));
        tiles(view, [168., 168.]);
    }
    check("second image metadata");
    for model in &models {
        model.set_vec(Vec::new());
    }
    check("retire image trees");
    for model in &models {
        model.push(picture(0, 54., false));
    }
    check("first image");
    for model in &models {
        model.push(picture(1, 272., true));
    }
    check("second image");
    check_colour(&windows[0], [0, 0, 255, 255], "red image");
    check_colour(&windows[0], [255, 0, 0, 255], "blue image");
    for view in &views {
        // It moves again below before snapshotting pending window damage.
        view.set_article_body_height(610.);
        view.set_article_load_available(true);
        view.set_article_load_status("Article loaded · synthetic 23 lines".into());
        view.set_article_updates_toggle_label("Update replies".into());
    }
    check("loaded before reply capability");
    let before_reply_controls = views.each_ref().map(geometry);
    for view in &views {
        view.set_article_updates_available(true);
    }
    check("new floating controls");
    assert_eq!(
        views.each_ref().map(geometry),
        before_reply_controls,
        "late reply capability moved/resized the acknowledged article viewport"
    );
    for (available, unread, recovery, busy) in [
        (false, 0, false, true),
        (true, 3, true, false),
        (true, 0, false, false),
    ] {
        for view in &views {
            view.set_article_load_available(available);
            view.set_article_updates_available(available);
            view.set_article_updates_unread(unread);
            view.set_article_updates_recovery(recovery);
            view.set_output_busy(busy);
        }
        check("reading capability and output busy changed");
        assert_eq!(
            views.each_ref().map(geometry),
            before_reply_controls,
            "reading controls moved/resized the acknowledged article viewport"
        );
    }
    let before_loading = views.each_ref().map(geometry);
    for active in [true, false] {
        for view in &views {
            view.set_article_load_active(active);
        }
        check("loading action changed");
        assert_eq!(
            views.each_ref().map(geometry),
            before_loading,
            "loading action moved/resized the acknowledged article viewport"
        );
    }
    for view in &views {
        // Keep this fixed synthetic footer inside the smallest logical audit
        // viewport (1920x1080 at 1.5 DPI).
        view.set_article_body_height(580.);
        // Diagnostic preparation can snapshot a newly installed projection
        // before it is painted into the window's retained backing buffer.
        // The snapshot must not consume the pending window damage.
        let snapshot = view.window().take_snapshot().unwrap();
        assert_eq!((snapshot.width(), snapshot.height()), (1040, 740));
    }
    check("footer geometry after offscreen snapshot");
    for frame in 0..100 {
        for model in &models {
            for row in 0..2 {
                let mut picture = model.row_data(row).unwrap();
                picture.image = solid(
                    512,
                    160,
                    if (frame + row) % 2 == 0 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    },
                );
                model.set_row_data(row, picture);
            }
        }
        check(&format!("animation frame {frame}"));
        let buffer = &windows[1].buffer;
        assert_eq!(buffer.last_age.get(), 1, "retained route was not exercised");
        let area = buffer
            .last_area
            .get()
            .expect("partial renderer did not return damage");
        let size = windows[1].size.get();
        assert!(
            area > 0. && area < (size.width * size.height) as f32,
            "not a partial update: {area}"
        );
    }
    for window in windows.iter() {
        window.resize(PhysicalSize::new(1920, 1080));
    }
    check("enlarge window");
    for scale_factor in [1.25, 1.5, 1.] {
        for window in windows.iter() {
            window
                .window
                .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });
            // The physical buffer stays unchanged. Logical geometry must be
            // invalidated without relying on a fresh allocation.
            window.resize(PhysicalSize::new(1920, 1080));
        }
        check("scale factor with unchanged physical buffer");
    }
    for window in windows.iter() {
        window.resize(PhysicalSize::new(1040, 740));
    }
    check("restore window");
    for model in &models {
        model.set_vec(Vec::new());
    }
    for view in &views {
        tiles(view, [0., 0.]);
    }
    check("remove both images");
    eprintln!(
        "offscreen independent Skia views: {} exact frames each, including100 partial image updates; not OS presentation",
        windows[0].buffer.frames.get()
    );
    for view in views {
        view.hide().unwrap();
    }
}
