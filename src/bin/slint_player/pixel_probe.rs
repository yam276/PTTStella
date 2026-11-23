//! Explicit diagnostic readback of four fixture pixels, never a frame bridge.
use super::gl_state::Saved;
use glow::HasContext;
use serde::Serialize;
use std::num::NonZeroU32;

#[derive(Serialize)]
pub struct Sample {
    size: [u32; 2],
    pixels: [[u8; 4]; 4],
    expected_rgb: [[u8; 3]; 4],
    pub passed: bool,
}

pub fn capture(gl: &glow::Context, width: u32, height: u32) -> Result<Sample, String> {
    if width < 64 || height < 64 || width > 8192 || height > 8192 {
        return Err("pixel probe dimensions outside 64..8192".into());
    }
    // SAFETY: only called from AfterRendering with the live current GL context.
    let saved = unsafe { Saved::take(gl)? };
    let target = saved.framebuffer();
    // SAFETY: the saved state belongs to this current context.
    unsafe { saved.defaults(gl) };
    let scale = (width as f64 / 320.).min(height as f64 * 0.7 / 180.);
    let video_w = 320. * scale;
    let video_h = 180. * scale;
    let left = (width as f64 - video_w) / 2.;
    let bottom = height as f64 * 0.2 + (height as f64 * 0.7 - video_h) / 2.;
    let mut pixels = [[0; 4]; 4];
    // SAFETY: called only during Slint AfterRendering with its current GL 3.3
    // context. The selected read FBO is its live draw FBO. PACK state was reset
    // and pixel-pack buffer unbound above: each 1x1 RGBA/UNSIGNED_BYTE read
    // writes exactly four bytes into its own four-byte destination. Coordinates
    // lie inside the validated drawable. No readback runs during normal frames.
    unsafe {
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            NonZeroU32::new(target as u32).map(glow::NativeFramebuffer),
        );
        let previous = gl.get_parameter_i32(glow::READ_BUFFER);
        gl.read_buffer(if target == 0 {
            glow::BACK
        } else {
            glow::COLOR_ATTACHMENT0
        });
        for (pixel, (x, y)) in
            pixels
                .iter_mut()
                .zip([(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)])
        {
            gl.read_pixels(
                (left + video_w * x) as i32,
                (bottom + video_h * (1. - y)) as i32,
                1,
                1,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(pixel)),
            );
        }
        gl.read_buffer(previous as u32);
    }
    // SAFETY: the same context remains current throughout this capture.
    unsafe { saved.restore(gl) };
    // Independently authored quadrant colors from the CC0 fixture definition.
    let expected_rgb = [[255, 0, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255]];
    let passed = pixels.iter().zip(expected_rgb).all(|(actual, expected)| {
        actual[..3]
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 30)
    });
    Ok(Sample {
        size: [width, height],
        pixels,
        expected_rgb,
        passed,
    })
}
