//! WebP pictures decoded with libwebp.
//!
//! Bluesky's picture server answers in WebP. The image crate's decoder took
//! 7 to 19 ms for a picture in a post and 135 to 150 ms for a full-size
//! photo; libwebp takes a third to a half of that, and it scales while it
//! decodes, so a photo larger than any box comes out at the size it is kept
//! at, in about 45 ms, without a second pass to shrink it.

use image::{DynamicImage, RgbImage, RgbaImage};
use libwebp_sys::{
    VP8StatusCode, WEBP_CSP_MODE, WebPDecode, WebPDecoderConfig, WebPFreeDecBuffer,
    WebPGetFeatures, WebPInitDecoderConfig,
};

/// Whether `bytes` start as a WebP file does.
pub fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// Decode the still WebP picture `bytes` at the size `size` gives for its
/// width and height (at most its own). `None` when it is not one libwebp
/// decodes this way (an animation, a damaged file), for the image crate to
/// try.
pub fn decode(bytes: &[u8], size: impl Fn(u32, u32) -> (u32, u32)) -> Option<DynamicImage> {
    if !is_webp(bytes) {
        return None;
    }
    // SAFETY: the config is initialized by libwebp before it is read; the
    // input slice outlives every call that reads it; the output buffer is
    // read within the size libwebp reports and freed exactly once.
    unsafe {
        let mut config: WebPDecoderConfig = std::mem::zeroed();
        if !WebPInitDecoderConfig(&mut config) {
            return None;
        }
        if WebPGetFeatures(bytes.as_ptr(), bytes.len(), &mut config.input)
            != VP8StatusCode::VP8_STATUS_OK
            || config.input.has_animation != 0
        {
            return None;
        }
        let (w, h) = (
            u32::try_from(config.input.width).ok()?,
            u32::try_from(config.input.height).ok()?,
        );
        let (ow, oh) = size(w, h);
        if ow == 0 || oh == 0 || ow > w || oh > h {
            return None;
        }
        if (ow, oh) != (w, h) {
            config.options.use_scaling = 1;
            config.options.scaled_width = i32::try_from(ow).ok()?;
            config.options.scaled_height = i32::try_from(oh).ok()?;
        }
        let alpha = config.input.has_alpha != 0;
        config.output.colorspace = if alpha {
            WEBP_CSP_MODE::MODE_RGBA
        } else {
            WEBP_CSP_MODE::MODE_RGB
        };
        if WebPDecode(bytes.as_ptr(), bytes.len(), &mut config) != VP8StatusCode::VP8_STATUS_OK {
            WebPFreeDecBuffer(&mut config.output);
            return None;
        }
        let out = config.output.u.RGBA;
        let row = ow as usize * if alpha { 4 } else { 3 };
        let stride = usize::try_from(out.stride).unwrap_or(0);
        let pixels =
            if out.rgba.is_null() || stride < row || out.size < stride * (oh as usize - 1) + row {
                None
            } else {
                let all = std::slice::from_raw_parts(out.rgba, out.size);
                let mut pixels = Vec::with_capacity(row * oh as usize);
                for y in 0..oh as usize {
                    pixels.extend_from_slice(&all[y * stride..y * stride + row]);
                }
                Some(pixels)
            };
        WebPFreeDecBuffer(&mut config.output);
        let pixels = pixels?;
        if alpha {
            RgbaImage::from_raw(ow, oh, pixels).map(DynamicImage::ImageRgba8)
        } else {
            RgbImage::from_raw(ow, oh, pixels).map(DynamicImage::ImageRgb8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// A picture of `w` x `h` with a gradient, as lossless WebP.
    fn webp(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([
                (x * 255 / w) as u8,
                (y * 255 / h) as u8,
                90,
                if alpha { (x % 256) as u8 } else { 255 },
            ])
        });
        let img = if alpha {
            DynamicImage::ImageRgba8(img)
        } else {
            DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(img).to_rgb8())
        };
        let mut out = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut out),
            image::ImageFormat::WebP,
        )
        .unwrap();
        out
    }

    #[rstest]
    #[case::rgb(false)]
    #[case::rgba(true)]
    fn a_webp_decodes_to_the_same_pixels_as_the_image_crate(#[case] alpha: bool) {
        let bytes = webp(37, 23, alpha);
        let ours = decode(&bytes, |w, h| (w, h)).unwrap();
        let theirs = image::load_from_memory(&bytes).unwrap();
        assert_eq!(ours.color(), theirs.color());
        assert_eq!(ours.as_bytes(), theirs.as_bytes());
    }

    #[test]
    fn a_webp_is_scaled_while_it_is_decoded() {
        let bytes = webp(400, 300, false);
        let img = decode(&bytes, |w, h| (w / 4, h / 4)).unwrap();
        assert_eq!((img.width(), img.height()), (100, 75));
    }

    #[rstest]
    #[case::png(&b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"[..])]
    #[case::short(&b"RIFF"[..])]
    #[case::damaged(&b"RIFF\x10\0\0\0WEBPVP8 \x04\0\0\0\0\0\0\0"[..])]
    #[case::empty(&b""[..])]
    fn what_is_not_a_readable_webp_is_left_to_the_image_crate(#[case] bytes: &[u8]) {
        assert!(decode(bytes, |w, h| (w, h)).is_none());
    }

    #[test]
    fn a_size_larger_than_the_picture_is_left_to_the_image_crate() {
        let bytes = webp(10, 10, false);
        assert!(decode(&bytes, |w, h| (w * 2, h * 2)).is_none());
        assert!(decode(&bytes, |_, _| (0, 5)).is_none());
    }
}
