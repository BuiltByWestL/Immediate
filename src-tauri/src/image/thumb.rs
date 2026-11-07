//! Small previews for the filmstrip. On Windows they come from the shell (the same cached
//! thumbnails Explorer shows, videos included; never generic file icons); elsewhere, or if that
//! fails, pictures are decoded and scaled down here. Files with no preview get none, and the
//! filmstrip shows their type instead.

use std::path::Path;

/// A PNG of the file, at most `size` pixels on its long side.
pub fn thumbnail(path: &Path, size: u32) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    if let Ok(img) = shell::thumbnail(path, size) {
        return encode(img);
    }
    if crate::formats::lookup(path).is_none_or(|f| f.kind != crate::formats::Kind::Image) {
        return Err("no preview".into());
    }
    let img = super::decode::decode(path)?;
    let full = image::RgbaImage::from_raw(img.w, img.h, img.data).ok_or("bad size")?;
    let (w, h) = (full.width(), full.height());
    let k = size as f32 / w.max(h) as f32;
    let small = if k < 1.0 {
        image::imageops::thumbnail(&full, ((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1))
    } else {
        full
    };
    encode(small)
}

fn encode(img: image::RgbaImage) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::NoFilter)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(windows)]
mod shell {
    use std::path::Path;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW, HGDIOBJ, ReleaseDC, BITMAP,
    };
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_RESIZETOFIT, SIIGBF_THUMBNAILONLY};
    use windows::core::HSTRING;

    pub fn thumbnail(path: &Path, size: u32) -> Result<image::RgbaImage, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).map_err(|e| e.to_string())?;
            let hbmp = factory
                .GetImage(SIZE { cx: size as i32, cy: size as i32 }, SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK | SIIGBF_THUMBNAILONLY)
                .map_err(|e| e.to_string())?;
            let obj = HGDIOBJ(hbmp.0);
            let mut bm = BITMAP::default();
            GetObjectW(obj, size_of::<BITMAP>() as i32, Some((&raw mut bm).cast()));
            let (w, h) = (bm.bmWidth.max(1) as u32, bm.bmHeight.unsigned_abs().max(1));
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32), // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut px = vec![0u8; (w * h * 4) as usize];
            let dc = GetDC(None);
            let lines = GetDIBits(dc, hbmp, 0, h, Some(px.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            ReleaseDC(None, dc);
            let _ = DeleteObject(obj);
            if lines == 0 {
                return Err("no thumbnail".into());
            }
            // BGRA → RGBA. Shell bitmaps without alpha report 0; treat as opaque.
            let opaque = px.chunks_exact(4).all(|p| p[3] == 0);
            for p in px.chunks_exact_mut(4) {
                p.swap(0, 2);
                if opaque {
                    p[3] = 255;
                } else if p[3] > 0 && p[3] < 255 {
                    // Shell bitmaps are premultiplied.
                    let a = p[3] as u32;
                    for c in &mut p[..3] {
                        *c = ((*c as u32 * 255) / a).min(255) as u8;
                    }
                }
            }
            image::RgbaImage::from_raw(w, h, px).ok_or("bad size".into())
        }
    }
}
