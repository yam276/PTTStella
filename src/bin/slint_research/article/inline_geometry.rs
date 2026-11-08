//! Crop in source pixels before narrowing to Slint's viewport-local f32 values.
#[derive(Debug)]
pub(super) struct Crop {
    pub top: f32,
    pub height: f32,
    pub image_y: f32,
    pub image_height: f32,
    pub source_y: i32,
    pub source_height: i32,
}
pub(super) fn crop(top: f64, height: f64, viewport: f64, source_height: u32) -> Option<Crop> {
    if !top.is_finite()
        || !height.is_finite()
        || !viewport.is_finite()
        || height <= 0.
        || viewport <= 0.
        || source_height == 0
    {
        return None;
    }
    let visible_top = top.max(0.);
    let visible_bottom = (top + height).min(viewport);
    if visible_bottom <= visible_top {
        return None;
    }
    let scale = height / f64::from(source_height);
    let first = ((visible_top - top) / scale).floor().max(0.) as u32;
    let last = (((visible_bottom - top) / scale).ceil() as u32).min(source_height);
    let source_height = last.checked_sub(first)?;
    Some(Crop {
        top: visible_top as f32,
        height: (visible_bottom - visible_top) as f32,
        image_y: (top + f64::from(first) * scale - visible_top) as f32,
        image_height: (f64::from(source_height) * scale) as f32,
        source_y: i32::try_from(first).ok()?,
        source_height: i32::try_from(source_height).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn very_tall_wide_image_keeps_only_local_source_rows() {
        // A 1x8192 image at 8160px width: scroll into source pixel 7000.
        let c = crop(-57_120_125., 66_846_720., 600., 8192).unwrap();
        assert_eq!(
            (c.top, c.height, c.source_y, c.source_height),
            (0., 600., 7000, 1)
        );
        assert_eq!((c.image_y, c.image_height), (-125., 8160.));
        assert_eq!(c.image_y + c.image_height, 8035.);
        assert!(crop(600., 32., 600., 32).is_none());
        assert!(crop(-32., 32., 600., 32).is_none());
    }
    #[test]
    fn fractional_crop_covers_viewport_without_stretching_source() {
        let c = crop(-10.25, 32., 16., 32).unwrap();
        assert_eq!(
            (c.top, c.height, c.source_y, c.source_height),
            (0., 16., 10, 17)
        );
        assert_eq!((c.image_y, c.image_height), (-0.25, 17.));
        let c = crop(10., 32., 600., 32).unwrap();
        assert_eq!(
            (c.top, c.height, c.image_y, c.source_y, c.source_height),
            (10., 32., 0., 0, 32)
        );
    }
}
