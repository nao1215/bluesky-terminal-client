//! Scaling a picture to the box it is drawn in, fast.
//!
//! ratatui-image scales every picture it encodes with the `image` crate,
//! which is slow at it: 19 ms to bring a 1280 x 720 video picture to a
//! full-screen box, most of the time a frame takes. [`to_box`] scales with
//! `fast_image_resize` (bilinear, the same filter family) to exactly the size
//! ratatui-image would pick, so its own scaling then finds nothing to do and
//! only copies. The size is computed the way `image` computes it, so what is
//! drawn is the same size as before, pixel for pixel.

use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::DynamicImage;
use ratatui::layout::Size;

/// Width and height of `w` x `h` fitted inside `bw` x `bh`, keeping its
/// shape: `image`'s `resize_dimensions` without filling.
fn fit(w: u32, h: u32, bw: u32, bh: u32) -> (u32, u32) {
    let ratio = f64::min(f64::from(bw) / f64::from(w), f64::from(bh) / f64::from(h));
    let nw = ((f64::from(w) * ratio).round() as u64).max(1);
    let nh = ((f64::from(h) * ratio).round() as u64).max(1);
    (
        nw.min(u64::from(u32::MAX)) as u32,
        nh.min(u64::from(u32::MAX)) as u32,
    )
}

/// The pixel size ratatui-image's `Resize::Scale` makes `(w, h)` for a box
/// of `area` cells of `cell` pixels: fitted in the box, rounded up to whole
/// cells, then fitted in those cells.
pub fn size_in_box(w: u32, h: u32, area: Size, cell: (u16, u16)) -> (u32, u32) {
    let (cw, ch) = (u32::from(cell.0.max(1)), u32::from(cell.1.max(1)));
    let (fw, fh) = fit(
        w,
        h,
        u32::from(area.width) * cw,
        u32::from(area.height) * ch,
    );
    let cols = fw.div_ceil(cw);
    let rows = fh.div_ceil(ch);
    fit(w, h, cols * cw, rows * ch)
}

/// `img` scaled to the size ratatui-image draws it at in `area`, or a copy
/// when it is that size already. A pixel format `fast_image_resize` does not
/// take is scaled by `image` instead, as before.
pub fn to_box(img: &DynamicImage, area: Size, cell: (u16, u16)) -> DynamicImage {
    if img.width() == 0 || img.height() == 0 || area.width == 0 || area.height == 0 {
        return img.clone();
    }
    let (w, h) = size_in_box(img.width(), img.height(), area, cell);
    if (w, h) == (img.width(), img.height()) {
        return img.clone();
    }
    let mut dst = DynamicImage::new(w, h, img.color());
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
    match Resizer::new().resize(img, &mut dst, &options) {
        Ok(()) => dst,
        Err(_) => img.resize_exact(w, h, image::imageops::FilterType::Triangle),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage, Rgba, RgbaImage};

    /// What ratatui-image asks `image` for: the picture fitted in the whole
    /// cells of the box it chose. `to_box` must land on exactly this size,
    /// so that request becomes a copy.
    fn ratatui_image_size(img: &DynamicImage, area: Size, cell: (u16, u16)) -> (u32, u32) {
        let (cw, ch) = (u32::from(cell.0), u32::from(cell.1));
        let fitted = img.resize(
            u32::from(area.width) * cw,
            u32::from(area.height) * ch,
            image::imageops::FilterType::Nearest,
        );
        let cols = fitted.width().div_ceil(cw);
        let rows = fitted.height().div_ceil(ch);
        img.resize(cols * cw, rows * ch, image::imageops::FilterType::Nearest)
            .dimensions()
    }

    #[test]
    fn the_size_is_the_one_ratatui_image_would_choose() {
        // Shapes rather than sizes: the arithmetic is the same at any size,
        // and the oracle resizes slowly in a debug build.
        let sizes = [
            (1, 1),
            (7, 3),
            (40, 40),
            (64, 48),
            (128, 72),
            (72, 128),
            (200, 150),
            (100, 400),
            (409, 17),
        ];
        let areas = [(1, 1), (4, 2), (36, 12), (69, 26), (160, 45), (13, 70)];
        for cell in [(10, 20), (8, 16), (7, 15)] {
            for &(w, h) in &sizes {
                let img = DynamicImage::new_luma8(w, h);
                for &(cols, rows) in &areas {
                    let area = Size::new(cols, rows);
                    assert_eq!(
                        size_in_box(w, h, area, cell),
                        ratatui_image_size(&img, area, cell),
                        "{w}x{h} in {cols}x{rows} cells of {cell:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_solid_picture_keeps_its_color_and_format() {
        let rgb = DynamicImage::ImageRgb8(RgbImage::from_pixel(300, 200, Rgb([10, 200, 30])));
        let out = to_box(&rgb, Size::new(20, 10), (10, 20));
        assert!(matches!(out, DynamicImage::ImageRgb8(_)));
        assert!(out.to_rgb8().pixels().all(|p| *p == Rgb([10, 200, 30])));

        // Alpha is scaled with the color, so a transparent picture stays
        // transparent and an opaque one opaque.
        let rgba = DynamicImage::ImageRgba8(RgbaImage::from_pixel(300, 200, Rgba([1, 2, 3, 0])));
        let out = to_box(&rgba, Size::new(20, 10), (10, 20));
        assert!(matches!(out, DynamicImage::ImageRgba8(_)));
        assert!(out.to_rgba8().pixels().all(|p| p[3] == 0));
    }

    #[test]
    fn the_scaled_picture_has_the_size_computed_for_it() {
        let img = DynamicImage::new_rgb8(1280, 720);
        for (cols, rows) in [(160, 45), (36, 12), (13, 70)] {
            let area = Size::new(cols, rows);
            let want = size_in_box(1280, 720, area, (10, 20));
            assert_eq!(to_box(&img, area, (10, 20)).dimensions(), want);
        }
    }

    #[test]
    fn a_picture_already_at_its_size_or_an_empty_box_is_left_alone() {
        let img = DynamicImage::new_rgb8(200, 100);
        assert_eq!(
            to_box(&img, Size::new(20, 5), (10, 20)).dimensions(),
            (200, 100)
        );
        assert_eq!(
            to_box(&img, Size::new(0, 5), (10, 20)).dimensions(),
            (200, 100)
        );
    }
}
