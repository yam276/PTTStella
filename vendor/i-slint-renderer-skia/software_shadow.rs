// Copyright © 2026 PTTStella contributors
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// One window's last successful software raster, independent of presentation.
#[derive(Default)]
pub(super) struct ShadowBuffer {
    size: (u32, u32),
    pixels: Vec<u8>,
    valid: bool,
}

impl ShadowBuffer {
    pub(super) fn prepare(&mut self, width: u32, height: u32, bytes: usize) -> (u8, &mut [u8]) {
        if self.size != (width, height) || self.pixels.len() != bytes {
            self.size = (width, height);
            // Replace rather than resize so restoring a smaller window releases
            // the fullscreen allocation. The existing target defines the size.
            self.pixels = vec![0; bytes];
            self.valid = false;
        }
        (u8::from(self.valid), &mut self.pixels)
    }

    pub(super) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(super) fn commit(&mut self) {
        self.valid = true;
    }

    pub(super) fn invalidate(&mut self) {
        self.valid = false;
    }
}

#[cfg(test)]
mod tests {
    use super::ShadowBuffer;

    #[test]
    fn preserves_unchanged_pixels_across_partial_rasters() {
        let mut buffer = ShadowBuffer::default();
        let (age, pixels) = buffer.prepare(2, 1, 8);
        assert_eq!(age, 0);
        pixels.copy_from_slice(&[1, 2, 3, 255, 4, 5, 6, 255]);
        buffer.commit();
        let (age, pixels) = buffer.prepare(2, 1, 8);
        assert_eq!(age, 1);
        pixels[..4].copy_from_slice(&[8, 9, 10, 255]);
        buffer.commit();
        assert_eq!(buffer.pixels(), &[8, 9, 10, 255, 4, 5, 6, 255]);
        // The handed-off copy is independent, including after another repaint.
        let presented = buffer.pixels().to_vec();
        let (_, pixels) = buffer.prepare(2, 1, 8);
        pixels.fill(11);
        assert_eq!(presented, [8, 9, 10, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn geometry_stride_and_failed_render_invalidate_history() {
        let mut buffer = ShadowBuffer::default();
        let (_, pixels) = buffer.prepare(2, 2, 16);
        pixels.fill(99);
        buffer.commit();
        // Equal byte length but a changed aspect ratio still needs a full frame.
        let (age, pixels) = buffer.prepare(4, 1, 16);
        assert_eq!(age, 0);
        assert_eq!(pixels, [0; 16]);
        buffer.commit();
        assert_eq!(buffer.prepare(4, 1, 16).0, 1);
        buffer.invalidate();
        assert_eq!(buffer.prepare(4, 1, 16).0, 0);
        buffer.commit();
        assert_eq!(buffer.prepare(4, 1, 32).0, 0);
        buffer.commit();
        let (age, pixels) = buffer.prepare(1, 1, 4);
        assert_eq!(age, 0);
        assert_eq!(pixels, [0; 4]);
        assert_eq!(buffer.pixels.capacity(), 4);
    }
}
