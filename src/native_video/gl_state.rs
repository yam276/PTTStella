//! Preserve the shared GL 3.3 state around libmpv's documented default-state API.
//! Only constructed/used in Slint's current-context rendering notifier.
use glow::HasContext;
use std::num::NonZeroU32;
pub struct Saved {
    enabled: Vec<(u32, bool)>,
    viewport: [i32; 4],
    scissor: [i32; 4],
    clear: [f32; 4],
    blend: [i32; 6],
    color: [bool; 4],
    depth: (i32, bool),
    pack: [i32; 8],
    framebuffer: (i32, i32),
    program: i32,
    vao: i32,
    buffers: [i32; 4],
    active: i32,
    textures: Vec<([i32; 3], i32)>,
}
const ENABLES: [u32; 9] = [
    glow::BLEND,
    glow::CULL_FACE,
    glow::DEPTH_TEST,
    glow::STENCIL_TEST,
    glow::SCISSOR_TEST,
    glow::DITHER,
    glow::FRAMEBUFFER_SRGB,
    glow::POLYGON_OFFSET_FILL,
    glow::RASTERIZER_DISCARD,
];
const PACK: [u32; 8] = [
    glow::PACK_ALIGNMENT,
    glow::UNPACK_ALIGNMENT,
    glow::PACK_ROW_LENGTH,
    glow::UNPACK_ROW_LENGTH,
    glow::UNPACK_SKIP_PIXELS,
    glow::UNPACK_SKIP_ROWS,
    glow::PACK_SKIP_PIXELS,
    glow::PACK_SKIP_ROWS,
];
const BLEND: [u32; 6] = [
    glow::BLEND_SRC_RGB,
    glow::BLEND_DST_RGB,
    glow::BLEND_SRC_ALPHA,
    glow::BLEND_DST_ALPHA,
    glow::BLEND_EQUATION_RGB,
    glow::BLEND_EQUATION_ALPHA,
];
const TEXTURES: [(u32, u32); 3] = [
    (glow::TEXTURE_2D, glow::TEXTURE_BINDING_2D),
    (glow::TEXTURE_3D, glow::TEXTURE_BINDING_3D),
    (glow::TEXTURE_RECTANGLE, glow::TEXTURE_BINDING_RECTANGLE),
];
impl Saved {
    /// # Safety
    /// gl's GL3.3 context must be current; restore in the same context while
    /// all captured Slint objects and bindings are still live.
    pub unsafe fn take(gl: &glow::Context) -> Result<Self, String> {
        // SAFETY: caller is the pinned GL 3.3+ notifier. All queried state is
        // defined by that API. Bindings refer to existing objects in this context.
        unsafe {
            let units = gl.get_parameter_i32(glow::MAX_COMBINED_TEXTURE_IMAGE_UNITS);
            if !(1..=256).contains(&units) {
                return Err("unsupported GL texture-unit count".into());
            }
            let active = gl.get_parameter_i32(glow::ACTIVE_TEXTURE);
            let mut textures = Vec::with_capacity(units as usize);
            for unit in 0..units {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                textures.push((
                    TEXTURES.map(|(_, binding)| gl.get_parameter_i32(binding)),
                    gl.get_parameter_i32(glow::SAMPLER_BINDING),
                ));
            }
            gl.active_texture(active as u32);
            let mut viewport = [0; 4];
            let mut scissor = [0; 4];
            let mut clear = [0.; 4];
            let mut color = [0; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
            gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut scissor);
            gl.get_parameter_f32_slice(glow::COLOR_CLEAR_VALUE, &mut clear);
            gl.get_parameter_i32_slice(glow::COLOR_WRITEMASK, &mut color);
            Ok(Self {
                enabled: ENABLES.map(|flag| (flag, gl.is_enabled(flag))).into(),
                viewport,
                scissor,
                clear,
                blend: BLEND.map(|flag| gl.get_parameter_i32(flag)),
                color: color.map(|b| b != 0),
                depth: (
                    gl.get_parameter_i32(glow::DEPTH_FUNC),
                    gl.get_parameter_i32(glow::DEPTH_WRITEMASK) != 0,
                ),
                pack: PACK.map(|flag| gl.get_parameter_i32(flag)),
                framebuffer: (
                    gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING),
                    gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING),
                ),
                program: gl.get_parameter_i32(glow::CURRENT_PROGRAM),
                vao: gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING),
                buffers: [
                    glow::ARRAY_BUFFER_BINDING,
                    glow::ELEMENT_ARRAY_BUFFER_BINDING,
                    glow::PIXEL_PACK_BUFFER_BINDING,
                    glow::PIXEL_UNPACK_BUFFER_BINDING,
                ]
                .map(|flag| gl.get_parameter_i32(flag)),
                active,
                textures,
            })
        }
    }
    pub fn framebuffer(&self) -> i32 {
        self.framebuffer.0
    }
    /// # Safety
    /// The context captured by take must be current on this thread.
    pub unsafe fn defaults(&self, gl: &glow::Context) {
        // SAFETY: same current context; only unbinding valid GL 3.3 resources.
        unsafe {
            for (flag, _) in &self.enabled {
                gl.disable(*flag);
            }
            gl.use_program(None);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            for unit in 0..self.textures.len() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                for (target, _) in TEXTURES {
                    gl.bind_texture(target, None);
                }
                gl.bind_sampler(unit as u32, None);
            }
            gl.active_texture(glow::TEXTURE0);
            gl.color_mask(true, true, true, true);
            gl.depth_mask(true);
            gl.blend_func(glow::ONE, glow::ZERO);
            gl.blend_equation(glow::FUNC_ADD);
            gl.clear_color(0., 0., 0., 0.);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
            gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
            gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
            gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
            gl.pixel_store_i32(glow::PACK_ALIGNMENT, 4);
            gl.pixel_store_i32(glow::PACK_ROW_LENGTH, 0);
            gl.pixel_store_i32(glow::PACK_SKIP_PIXELS, 0);
            gl.pixel_store_i32(glow::PACK_SKIP_ROWS, 0);
        }
    }
    /// # Safety
    /// Same current context as take; all captured bindings still live.
    pub unsafe fn restore(self, gl: &glow::Context) {
        // SAFETY: snapshots only contain live bindings from the same context.
        // The UI's objects remain owned by Slint throughout this callback.
        unsafe {
            gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                NonZeroU32::new(self.framebuffer.0 as u32).map(glow::NativeFramebuffer),
            );
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                NonZeroU32::new(self.framebuffer.1 as u32).map(glow::NativeFramebuffer),
            );
            gl.use_program(NonZeroU32::new(self.program as u32).map(glow::NativeProgram));
            gl.bind_vertex_array(NonZeroU32::new(self.vao as u32).map(glow::NativeVertexArray));
            for (target, value) in [
                glow::ARRAY_BUFFER,
                glow::ELEMENT_ARRAY_BUFFER,
                glow::PIXEL_PACK_BUFFER,
                glow::PIXEL_UNPACK_BUFFER,
            ]
            .into_iter()
            .zip(self.buffers)
            {
                if target != glow::ELEMENT_ARRAY_BUFFER || self.vao != 0 {
                    gl.bind_buffer(
                        target,
                        NonZeroU32::new(value as u32).map(glow::NativeBuffer),
                    );
                }
            }
            for (unit, (textures, sampler)) in self.textures.into_iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                for ((target, _), value) in TEXTURES.into_iter().zip(textures) {
                    gl.bind_texture(
                        target,
                        NonZeroU32::new(value as u32).map(glow::NativeTexture),
                    );
                }
                gl.bind_sampler(
                    unit as u32,
                    NonZeroU32::new(sampler as u32).map(glow::NativeSampler),
                );
            }
            gl.active_texture(self.active as u32);
            gl.viewport(
                self.viewport[0],
                self.viewport[1],
                self.viewport[2],
                self.viewport[3],
            );
            gl.scissor(
                self.scissor[0],
                self.scissor[1],
                self.scissor[2],
                self.scissor[3],
            );
            gl.clear_color(self.clear[0], self.clear[1], self.clear[2], self.clear[3]);
            gl.color_mask(self.color[0], self.color[1], self.color[2], self.color[3]);
            gl.depth_func(self.depth.0 as u32);
            gl.depth_mask(self.depth.1);
            gl.blend_func_separate(
                self.blend[0] as u32,
                self.blend[1] as u32,
                self.blend[2] as u32,
                self.blend[3] as u32,
            );
            gl.blend_equation_separate(self.blend[4] as u32, self.blend[5] as u32);
            for (flag, value) in PACK.into_iter().zip(self.pack) {
                gl.pixel_store_i32(flag, value);
            }
            for (flag, enabled) in self.enabled {
                if enabled {
                    gl.enable(flag)
                } else {
                    gl.disable(flag)
                }
            }
        }
    }
}
