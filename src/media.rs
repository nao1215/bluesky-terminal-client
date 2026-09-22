//! Local pictures: reading one the way it is meant to be seen, and making it
//! fit what a post may carry.
//!
//! Every attached picture is decoded and encoded again rather than uploaded
//! as it is. That turns a photo taken sideways upright (the camera's EXIF
//! orientation), brings it under the size Bluesky accepts, and leaves the
//! camera's metadata, location included, on the user's disk.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbImage};

use crate::error::{Error, Result};

/// Most bytes an image of a post may have.
pub const MAX_POST_IMAGE_BYTES: usize = 1_000_000;
/// Most images a post may have.
pub const MAX_POST_IMAGES: usize = 4;
/// Longest side, in pixels, of an uploaded image; larger ones are scaled
/// down, which Bluesky's own clients also do.
const MAX_SIDE: u32 = 2000;
/// Largest file bs reads as a picture.
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
/// File name extensions shown as pictures.
const EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

/// Whether a file name looks like a picture bs can read.
pub fn is_image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// A picture ready to upload.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Read a picture upright. The error names the file and what is wrong.
pub fn load(path: &Path) -> Result<(DynamicImage, ImageFormat)> {
    let shown = path.display();
    let file = File::open(path).map_err(|e| Error::io(format!("cannot read {shown}: {e}")))?;
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len > MAX_FILE_BYTES {
        return Err(Error::io(format!(
            "{shown} is {} MB; pictures must be at most {} MB",
            len / (1024 * 1024),
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let not_image = |e: &dyn std::fmt::Display| {
        Error::io(format!(
            "{shown} is not a picture bs can read (PNG, JPEG, GIF, WebP): {e}"
        ))
    };
    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|e| not_image(&e))?;
    let format = reader
        .format()
        .ok_or_else(|| not_image(&"unknown format"))?;
    let mut decoder = reader.into_decoder().map_err(|e| not_image(&e))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| not_image(&e))?;
    img.apply_orientation(orientation);
    Ok((img, format))
}

/// Read `path` and encode it for a post.
pub fn prepare(path: &Path) -> Result<Prepared> {
    let (img, format) = load(path)?;
    encode_within(img, format, MAX_SIDE, MAX_POST_IMAGE_BYTES).ok_or_else(|| {
        Error::io(format!(
            "{} cannot be made smaller than {MAX_POST_IMAGE_BYTES} bytes",
            path.display()
        ))
    })
}

/// Read `path` and encode it as an avatar: upright, at most 1000 pixels on
/// a side, a PNG or JPEG under `limit` bytes, without its metadata.
pub fn prepare_avatar(path: &Path, limit: usize) -> Result<Prepared> {
    let (img, format) = load(path)?;
    encode_within(img, format, 1000, limit).ok_or_else(|| {
        Error::io(format!(
            "{} cannot be made smaller than {limit} bytes",
            path.display()
        ))
    })
}

/// Encode `img` in at most `limit` bytes, its longest side at most
/// `max_side`. A PNG or GIF (screenshots, drawings) is kept lossless when it
/// fits, and so is a picture with transparency; otherwise it becomes a JPEG
/// of the best quality that fits, and then a smaller one.
fn encode_within(
    img: DynamicImage,
    format: ImageFormat,
    max_side: u32,
    limit: usize,
) -> Option<Prepared> {
    // Eight bits a channel: a 16-bit PNG would otherwise stay twice the size
    // it needs to be, and JPEG has no other depth.
    let img = match img {
        DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageRgba8(_)
        | DynamicImage::ImageLuma8(_)
        | DynamicImage::ImageLumaA8(_) => img,
        other if other.color().has_alpha() => DynamicImage::ImageRgba8(other.to_rgba8()),
        other => DynamicImage::ImageRgb8(other.to_rgb8()),
    };
    let mut img = shrink(img, max_side);
    let lossless_first = matches!(format, ImageFormat::Png | ImageFormat::Gif);
    for _ in 0..4 {
        if lossless_first || img.color().has_alpha() {
            let mut png = Vec::new();
            if img
                .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
                .is_ok()
                && png.len() <= limit
            {
                return Some(prepared(png, "image/png", &img));
            }
        }
        let rgb = flatten(&img);
        for quality in [90, 80, 70, 60, 50] {
            let mut jpeg = Vec::new();
            if JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode_image(&rgb)
                .is_ok()
                && jpeg.len() <= limit
            {
                return Some(prepared(jpeg, "image/jpeg", &img));
            }
        }
        let (w, h) = (img.width() * 3 / 4, img.height() * 3 / 4);
        if w == 0 || h == 0 {
            break;
        }
        img = img.resize_exact(w, h, FilterType::CatmullRom);
    }
    None
}

fn prepared(bytes: Vec<u8>, mime: &'static str, img: &DynamicImage) -> Prepared {
    Prepared {
        bytes,
        mime,
        width: img.width(),
        height: img.height(),
    }
}

/// Scale down so the longest side is at most `max_side`.
fn shrink(img: DynamicImage, max_side: u32) -> DynamicImage {
    if img.width().max(img.height()) <= max_side {
        return img;
    }
    img.resize(max_side, max_side, FilterType::CatmullRom)
}

/// The picture on white, for a format without transparency.
fn flatten(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over = |c: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
        image::Rgb([over(r), over(g), over(b)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, Rgba, RgbaImage};
    use rstest::rstest;

    /// A picture that compresses badly, so the size limit has work to do.
    fn noise(w: u32, h: u32) -> DynamicImage {
        let mut seed = 0x2545_f491_u32;
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |_, _| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let [a, b, c, _] = seed.to_le_bytes();
            Rgb([a, b, c])
        }))
    }

    #[rstest]
    #[case("photo.JPG", true)]
    #[case("shot.png", true)]
    #[case("anim.gif", true)]
    #[case("a.webp", true)]
    #[case("notes.txt", false)]
    #[case("png", false)]
    #[case(".jpeg", false)]
    fn picture_names_are_recognized(#[case] name: &str, #[case] want: bool) {
        assert_eq!(is_image_name(name), want);
    }

    #[test]
    fn a_small_png_stays_a_png_at_its_size() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb([10, 20, 30])));
        let p = encode_within(img, ImageFormat::Png, 2000, 1_000_000).unwrap();
        assert_eq!((p.mime, p.width, p.height), ("image/png", 40, 30));
        assert_eq!(&p.bytes[..4], b"\x89PNG");
    }

    #[test]
    fn a_large_picture_is_scaled_to_the_longest_side() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(300, 100, Rgb([1, 2, 3])));
        let p = encode_within(img, ImageFormat::Jpeg, 120, 1_000_000).unwrap();
        assert_eq!((p.mime, p.width, p.height), ("image/jpeg", 120, 40));
    }

    #[test]
    fn a_picture_over_the_limit_becomes_a_smaller_jpeg() {
        let p = encode_within(noise(200, 150), ImageFormat::Png, 2000, 20_000).unwrap();
        assert!(p.bytes.len() <= 20_000, "{} bytes", p.bytes.len());
        assert_eq!(p.mime, "image/jpeg");
        let ratio = f64::from(p.width) / f64::from(p.height);
        assert!(
            (ratio - 4.0 / 3.0).abs() < 0.02,
            "the shape is kept: {ratio}"
        );
    }

    #[test]
    fn a_16_bit_picture_is_encoded_at_8_bits() {
        let img =
            DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(8, 8, Rgb([65535u16, 0, 0])));
        let p = encode_within(img, ImageFormat::Png, 2000, 1_000_000).unwrap();
        let back = image::load_from_memory(&p.bytes).unwrap();
        assert_eq!(back.color(), image::ColorType::Rgb8);
    }

    #[test]
    fn an_avatar_is_scaled_and_kept_under_its_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("me.webp");
        DynamicImage::ImageRgb8(RgbImage::from_pixel(1200, 900, Rgb([7, 8, 9])))
            .save(&path)
            .unwrap();
        let p = prepare_avatar(&path, 200_000).unwrap();
        assert_eq!((p.width, p.height), (1000, 750));
        assert!(p.bytes.len() <= 200_000);
        assert!(p.mime == "image/png" || p.mime == "image/jpeg");
    }

    #[test]
    fn no_encoding_small_enough_is_none() {
        assert!(encode_within(noise(64, 64), ImageFormat::Png, 2000, 10).is_none());
    }

    #[test]
    fn transparency_is_kept_in_a_png_or_flattened_on_white() {
        let clear = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0])));
        let p = encode_within(clear.clone(), ImageFormat::WebP, 2000, 1_000_000).unwrap();
        assert_eq!(p.mime, "image/png");
        assert_eq!(flatten(&clear).get_pixel(0, 0).0, [255, 255, 255]);
    }

    #[test]
    fn a_picture_file_is_read_and_prepared() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.png");
        DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 48, Rgb([200, 0, 0])))
            .save(&path)
            .unwrap();
        let p = prepare(&path).unwrap();
        assert_eq!((p.mime, p.width, p.height), ("image/png", 64, 48));
    }

    #[test]
    fn a_missing_or_non_picture_file_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone.png");
        let e = prepare(&missing).unwrap_err();
        assert!(e.message().contains("cannot read") && e.message().contains("gone.png"));
        let text = dir.path().join("notes.png");
        std::fs::write(&text, "not a picture").unwrap();
        let e = prepare(&text).unwrap_err();
        assert!(
            e.message().contains("notes.png is not a picture"),
            "{}",
            e.message()
        );
    }
}
