//! Decoding pictures the webview can't show itself: TIFF, TGA, PNM, QOI, HDR, EXR, DDS (image
//! crate), JPEG XL (jxl-oxide), Photoshop (psd), camera RAW (rawler: the camera's full-size
//! preview), and on Windows anything else Windows can open (HEIC/HEIF via WIC).
//! Output is upright 8-bit RGBA in sRGB.

use std::path::Path;

pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

const RAW: &[&str] = &[
    "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "3fr", "iiq",
];

pub fn decode(path: &Path) -> Result<Rgba, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let primary = match ext.as_str() {
        e if RAW.contains(&e) => raw_preview(path),
        "psd" | "psb" => photoshop(path),
        "jxl" => jpeg_xl(path),
        "heic" | "heif" | "avif" => Err(String::new()),
        _ => with_image_crate(path),
    };
    match primary {
        Ok(img) => Ok(img),
        // Whatever Immediate can't read itself, Windows may (HEIC with the HEIF extension,
        // more RAW cameras with the Raw Image Extension, JPEG XL with its extension…).
        #[cfg(windows)]
        Err(first) => wic::decode(path).map_err(|e| if first.is_empty() { e } else { first }),
        #[cfg(not(windows))]
        Err(e) => Err(if e.is_empty() { format!("Immediate can’t open .{ext} pictures on this system yet.") } else { e }),
    }
}

fn from_dynamic(img: image::DynamicImage) -> Rgba {
    use image::DynamicImage::*;
    let img = match img {
        // High dynamic range: compress highlights instead of clipping, then encode as sRGB.
        ImageRgb32F(_) | ImageRgba32F(_) => {
            let mut f = img.to_rgba32f();
            for p in f.pixels_mut() {
                for c in &mut p.0[..3] {
                    let x = c.max(0.0);
                    let t = x * (1.0 + x / 16.0) / (1.0 + x);
                    *c = if t <= 0.003_130_8 { t * 12.92 } else { 1.055 * t.powf(1.0 / 2.4) - 0.055 };
                }
            }
            image::DynamicImage::ImageRgba32F(f)
        }
        other => other,
    };
    let rgba = img.to_rgba8();
    Rgba { w: rgba.width(), h: rgba.height(), data: rgba.into_raw() }
}

fn with_image_crate(path: &Path) -> Result<Rgba, String> {
    use image::ImageDecoder;
    let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(from_dynamic(img))
}

fn raw_preview(path: &Path) -> Result<Rgba, String> {
    use rawler::decoders::RawDecodeParams;
    let src = rawler::rawsource::RawSource::new(path).map_err(|e| e.to_string())?;
    let decoder = rawler::get_decoder(&src).map_err(|e| e.to_string())?;
    let params = RawDecodeParams::default();
    let img = decoder
        .full_image(&src, &params)
        .ok()
        .flatten()
        .or_else(|| decoder.preview_image(&src, &params).ok().flatten())
        .ok_or("This camera file has no preview Immediate can show.")?;
    let mut img = image::DynamicImage::from(img);
    if let Some(o) = decoder.raw_metadata(&src, &params).ok().and_then(|m| m.exif.orientation) {
        if let Some(o) = image::metadata::Orientation::from_exif(o as u8) {
            img.apply_orientation(o);
        }
    }
    Ok(from_dynamic(img))
}

fn photoshop(path: &Path) -> Result<Rgba, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let psd = psd::Psd::from_bytes(&bytes).map_err(|e| format!("This Photoshop file can’t be read: {e}"))?;
    Ok(Rgba { w: psd.width(), h: psd.height(), data: psd.rgba() })
}

fn jpeg_xl(path: &Path) -> Result<Rgba, String> {
    use jxl_oxide::{EnumColourEncoding, JxlImage, RenderingIntent};
    let mut img = JxlImage::builder().open(path).map_err(|e| e.to_string())?;
    img.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
    let render = img.render_frame(0).map_err(|e| e.to_string())?;
    let fb = render.image_all_channels();
    let (w, h, ch) = (fb.width(), fb.height(), fb.channels());
    let src = fb.buf();
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let mut data = Vec::with_capacity(w * h * 4);
    for px in src.chunks_exact(ch) {
        let (r, g, b, a) = match ch {
            1 => (px[0], px[0], px[0], 1.0),
            2 => (px[0], px[0], px[0], px[1]),
            3 => (px[0], px[1], px[2], 1.0),
            _ => (px[0], px[1], px[2], px[3]),
        };
        data.extend([to8(r), to8(g), to8(b), to8(a)]);
    }
    Ok(Rgba { w: w as u32, h: h as u32, data })
}

#[cfg(windows)]
mod wic {
    use super::Rgba;
    use std::path::Path;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::*;
    use windows::core::HSTRING;

    pub fn decode(path: &Path) -> Result<Rgba, String> {
        let ext = path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
        let unsupported = || {
            if ext == "HEIC" || ext == "HEIF" {
                "Windows can’t open HEIC pictures on this PC. Install “HEIF Image Extensions” and “HEVC Video Extensions” from the Microsoft Store, then try again.".to_owned()
            } else {
                format!("Immediate can’t open .{ext} pictures on this PC yet.")
            }
        };
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
            let decoder = factory
                .CreateDecoderFromFilename(&HSTRING::from(path.as_os_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
                .map_err(|_| unsupported())?;
            let frame = decoder.GetFrame(0).map_err(|_| unsupported())?;
            let source: IWICBitmapSource = WICConvertBitmapSource(&GUID_WICPixelFormat32bppRGBA, &frame).map_err(|_| unsupported())?;
            let (mut w, mut h) = (0u32, 0u32);
            source.GetSize(&mut w, &mut h).map_err(|e| e.to_string())?;
            let mut data = vec![0u8; w as usize * h as usize * 4];
            source.CopyPixels(std::ptr::null(), w * 4, &mut data).map_err(|_| unsupported())?;
            let mut img = image::RgbaImage::from_raw(w, h, data).ok_or("bad size")?;
            // Upright, as the camera meant it.
            let orientation = frame.GetMetadataQueryReader().ok().and_then(|q| {
                let mut v = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::default();
                q.GetMetadataByName(&HSTRING::from("System.Photo.Orientation"), &mut v).ok()?;
                u16::try_from(&v).ok()
            });
            if let Some(o) = orientation.and_then(|o| image::metadata::Orientation::from_exif(o as u8)) {
                let mut d = image::DynamicImage::ImageRgba8(img);
                d.apply_orientation(o);
                img = d.to_rgba8();
            }
            Ok(Rgba { w: img.width(), h: img.height(), data: img.into_raw() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_tiff_with_the_image_crate() {
        let dir = std::env::temp_dir().join(format!("immediate-decode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.tif");
        image::RgbImage::from_fn(5, 3, |x, _| image::Rgb([x as u8 * 50, 0, 0])).save(&p).unwrap();
        let d = decode(&p).unwrap();
        assert_eq!((d.w, d.h), (5, 3));
        assert_eq!(&d.data[4..8], &[50, 0, 0, 255]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
