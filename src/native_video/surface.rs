//! GPU-only intermediate surface. The render notifier owns allocation, use and
//! destruction on its current GL context; no video pixels cross the CPU bridge.
use super::mpv;
use glow::HasContext;

pub struct Surface {
    texture: glow::NativeTexture,
    framebuffer: glow::NativeFramebuffer,
    size: [i32; 2],
}
impl Surface {
    /// # Safety
    /// The caller's GL3.3 context is current for this call and all later uses;
    /// destroy the surface explicitly on that context before it is torn down.
    pub unsafe fn new(gl: &glow::Context, width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 || !super::render_size_allowed(width as u32, height as u32) {
            return Err("video surface exceeds pixel budget".into());
        }
        // SAFETY: caller guarantees the current context. Partial allocations
        // are deleted before returning errors. All parameters are bounded above.
        unsafe {
            let texture = gl.create_texture()?;
            let framebuffer = match gl.create_framebuffer() {
                Ok(fbo) => fbo,
                Err(error) => {
                    gl.delete_texture(texture);
                    return Err(error);
                }
            };
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                gl.delete_framebuffer(framebuffer);
                gl.delete_texture(texture);
                return Err("native video framebuffer incomplete".into());
            }
            gl.viewport(0, 0, width, height);
            gl.clear_color(0., 0., 0., 1.);
            gl.clear(glow::COLOR_BUFFER_BIT);
            Ok(Self {
                texture,
                framebuffer,
                size: [width, height],
            })
        }
    }
    pub fn size(&self) -> [i32; 2] {
        self.size
    }
    pub fn framebuffer(&self) -> i32 {
        self.framebuffer.0.get() as i32
    }
    /// # Safety
    /// Same current context as creation, with libmpv's default GL state set.
    pub unsafe fn draw(&self, gl: &glow::Context, player: &mpv::Render) -> Result<(), String> {
        // SAFETY: owned complete FBO with exactly these validated dimensions.
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
            player.draw(self.framebuffer(), self.size[0], self.size[1])
        }
    }
    /// # Safety
    /// Same current context; target is Slint's live draw framebuffer. Bounds
    /// use physical pixels, bottom-left origin, inside its current drawable.
    pub unsafe fn blit(&self, gl: &glow::Context, target: i32, bounds: [i32; 4]) {
        // SAFETY: distinct live framebuffers, no client memory or CPU copy.
        unsafe {
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.framebuffer));
            gl.read_buffer(glow::COLOR_ATTACHMENT0);
            gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                std::num::NonZeroU32::new(target as u32).map(glow::NativeFramebuffer),
            );
            gl.blit_framebuffer(
                0,
                0,
                self.size[0],
                self.size[1],
                bounds[0],
                bounds[1],
                bounds[2],
                bounds[3],
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
        }
    }
    /// # Safety
    /// Same current context as creation; no further uses may follow.
    pub unsafe fn destroy(self, gl: &glow::Context) {
        // SAFETY: unique owner deletes exactly its two allocations once.
        unsafe {
            gl.delete_framebuffer(self.framebuffer);
            gl.delete_texture(self.texture);
        }
    }
}
