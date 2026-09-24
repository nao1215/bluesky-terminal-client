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

/// `img` (as [`to_box`] made it for `area`) on the transparent canvas of
/// whole cells that ratatui-image's `Resize::Scale` would lay it on, when
/// that canvas is larger than the picture.
///
/// ratatui-image lays a picture that does not fill whole cells on such a
/// canvas itself, blending pixel by pixel through the generic image API:
/// 1.75 ms for an 800 x 450 video picture in an 80 x 24 box, more than the
/// sixel, iTerm2 or half-block encode after it takes for its own part. Here
/// rows are copied, and ratatui-image finds a picture of the size it wants
/// and only copies it. The pixels are the ones its blend gives: an opaque
/// pixel as it is, a clear one left clear, any other through the same
/// blend. A picture of another pixel format, or one ratatui-image would
/// scale again first, is returned as it is, for ratatui-image to handle as
/// before.
pub fn pad_to_cells(img: DynamicImage, area: Size, cell: (u16, u16)) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 || area.width == 0 || area.height == 0 {
        return img;
    }
    let (cw, ch) = (u32::from(cell.0.max(1)), u32::from(cell.1.max(1)));
    let (bw, bh) = (u32::from(area.width) * cw, u32::from(area.height) * ch);
    // The cells ratatui-image picks: the picture fitted in the box, rounded
    // up to whole cells, as its `round_pixel_size_to_cells` rounds.
    let (fw, fh) = fit(w, h, bw, bh);
    let cols = (fw as f32 / cw as f32).ceil() as u32;
    let rows = (fh as f32 / ch as f32).ceil() as u32;
    let (pw, ph) = (cols * cw, rows * ch);
    // Already whole cells (nothing to lay it on), or a size ratatui-image
    // scales again before laying it down: left to it.
    if (pw, ph) == (w, h) || fit(w, h, pw, ph) != (w, h) {
        return img;
    }
    // The canvas must in turn be what ratatui-image keeps as it is.
    if fit(pw, ph, bw, bh) != (pw, ph) {
        return img;
    }
    let (w, pw, ph) = (w as usize, pw as usize, ph as usize);
    let mut out = vec![0u8; pw * ph * 4];
    match &img {
        DynamicImage::ImageRgb8(src) => {
            for (row, dst) in src
                .as_raw()
                .chunks_exact(w * 3)
                .zip(out.chunks_exact_mut(pw * 4))
            {
                for (s, d) in row
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .zip(dst.as_chunks_mut::<4>().0)
                {
                    *d = [s[0], s[1], s[2], 255];
                }
            }
        }
        DynamicImage::ImageRgba8(src) => {
            use image::Pixel;
            for (row, dst) in src
                .as_raw()
                .chunks_exact(w * 4)
                .zip(out.chunks_exact_mut(pw * 4))
            {
                for (s, d) in row
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(dst.as_chunks_mut::<4>().0)
                {
                    match s[3] {
                        0 => {}
                        255 => *d = *s,
                        _ => {
                            let mut p = image::Rgba([0u8; 4]);
                            p.blend(&image::Rgba(*s));
                            *d = p.0;
                        }
                    }
                }
            }
        }
        _ => return img,
    }
    match image::RgbaImage::from_raw(pw as u32, ph as u32, out) {
        Some(buf) => DynamicImage::ImageRgba8(buf),
        None => img,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage, Rgba, RgbaImage};
    use ratatui_image::picker::{Picker, ProtocolType};
    use ratatui_image::protocol::Protocol;
    use ratatui_image::{FilterType as RFilter, Resize};

    /// What ratatui-image sends for `img` in `area`: the iTerm2 sequence
    /// holds the PNG of exactly the pixels it would draw, and its size.
    fn sent(img: DynamicImage, area: Size) -> (String, Size) {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((10, 20).into());
        picker.set_protocol_type(ProtocolType::Iterm2);
        match picker
            .new_protocol(img, area, Resize::Scale(Some(RFilter::Triangle)))
            .unwrap()
        {
            Protocol::ITerm2(p) => (p.data, p.size),
            _ => unreachable!(),
        }
    }

    /// Laying a picture on its cells beforehand sends the same bytes as
    /// leaving it to ratatui-image, for pictures of every shape, clear,
    /// half-clear and opaque pixels, and the formats it does not handle.
    #[test]
    fn a_picture_laid_on_its_cells_is_sent_as_before() {
        let sizes = [
            (1, 1),
            (7, 3),
            (64, 36),
            (36, 64),
            (41, 17),
            (200, 150),
            (13, 90),
        ];
        let areas = [(1u16, 1u16), (4, 2), (8, 3), (13, 7), (6, 9)];
        let mut padded = 0;
        for &(w, h) in &sizes {
            let rgba = RgbaImage::from_fn(w, h, |x, y| {
                let a = [0, 255, 128, 1, 254][((x + y) % 5) as usize];
                Rgba([(x * 7) as u8, (y * 13) as u8, (x ^ y) as u8, a])
            });
            let pictures = [
                DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(rgba.clone()).to_rgb8()),
                DynamicImage::ImageRgba8(rgba.clone()),
                DynamicImage::ImageLuma8(DynamicImage::ImageRgba8(rgba).to_luma8()),
            ];
            for img in pictures {
                for &(cols, rows) in &areas {
                    let area = Size::new(cols, rows);
                    let boxed = to_box(&img, area, (10, 20));
                    let laid = pad_to_cells(boxed.clone(), area, (10, 20));
                    if laid.dimensions() != boxed.dimensions() {
                        padded += 1;
                    }
                    assert_eq!(
                        sent(laid, area),
                        sent(boxed, area),
                        "{w}x{h} {:?} in {cols}x{rows} cells",
                        img.color()
                    );
                }
            }
        }
        assert!(padded > 20, "only {padded} pictures were laid on cells");
    }

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
