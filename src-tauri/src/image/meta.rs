//! Photo metadata that has to survive an edit: EXIF (with orientation fixed, the stale embedded
//! thumbnail removed, and location removed on request), the ICC color profile, and JPEG quality.

/// A TIFF structure (EXIF is one) that can be patched in place.
struct Tiff<'a> {
    b: &'a mut [u8],
    le: bool,
}

const TYPE_SIZES: [usize; 13] = [0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8, 4, 8];

impl<'a> Tiff<'a> {
    fn new(b: &'a mut [u8]) -> Option<Self> {
        let le = match b.get(..4)? {
            [b'I', b'I', 42, 0] => true,
            [b'M', b'M', 0, 42] => false,
            _ => return None,
        };
        Some(Self { b, le })
    }
    fn u16(&self, at: usize) -> Option<u16> {
        let s: [u8; 2] = self.b.get(at..at + 2)?.try_into().ok()?;
        Some(if self.le { u16::from_le_bytes(s) } else { u16::from_be_bytes(s) })
    }
    fn u32(&self, at: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(at..at + 4)?.try_into().ok()?;
        Some(if self.le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    }
    fn put_u16(&mut self, at: usize, v: u16) {
        let s = if self.le { v.to_le_bytes() } else { v.to_be_bytes() };
        if let Some(d) = self.b.get_mut(at..at + 2) {
            d.copy_from_slice(&s);
        }
    }
    fn put_u32(&mut self, at: usize, v: u32) {
        let s = if self.le { v.to_le_bytes() } else { v.to_be_bytes() };
        if let Some(d) = self.b.get_mut(at..at + 4) {
            d.copy_from_slice(&s);
        }
    }
    fn zero(&mut self, at: usize, len: usize) {
        let end = (at + len).min(self.b.len());
        if at < end {
            self.b[at..end].fill(0);
        }
    }
    fn ifd0(&self) -> Option<usize> {
        self.u32(4).map(|o| o as usize)
    }
    /// Entries of the IFD at `ifd`: (entry offset, tag, type, count).
    fn entries(&self, ifd: usize) -> Vec<(usize, u16, u16, u32)> {
        let Some(n) = self.u16(ifd) else { return vec![] };
        (0..n as usize)
            .filter_map(|i| {
                let e = ifd + 2 + i * 12;
                Some((e, self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)?))
            })
            .collect()
    }
    fn find(&self, ifd: usize, tag: u16) -> Option<(usize, u16, u32)> {
        self.entries(ifd).into_iter().find(|e| e.1 == tag).map(|(e, _, t, c)| (e, t, c))
    }
    fn next_ifd_at(&self, ifd: usize) -> Option<usize> {
        Some(ifd + 2 + self.u16(ifd)? as usize * 12)
    }
    /// Sets a SHORT or LONG value stored inline in an entry.
    fn set_inline(&mut self, entry: usize, typ: u16, v: u32) {
        match typ {
            3 => self.put_u16(entry + 8, v as u16),
            4 => self.put_u32(entry + 8, v),
            _ => {}
        }
    }
}

pub struct ExifPatch {
    pub width: u32,
    pub height: u32,
    pub strip_location: bool,
}

const ORIENTATION: u16 = 0x0112;
const EXIF_IFD: u16 = 0x8769;
const GPS_IFD: u16 = 0x8825;
const PIXEL_X: u16 = 0xA002;
const PIXEL_Y: u16 = 0xA003;
const THUMB_OFFSET: u16 = 0x0201;
const THUMB_LEN: u16 = 0x0202;

/// Returns EXIF (TIFF bytes, without the "Exif\0\0" prefix) fixed up for the edited pixels:
/// orientation = upright (the edit bakes rotation in), new pixel dimensions, no embedded
/// thumbnail (it would show the unedited photo), and optionally no GPS location at all.
pub fn patch_exif(exif: &[u8], p: &ExifPatch) -> Option<Vec<u8>> {
    let mut data = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif).to_vec();
    let mut t = Tiff::new(&mut data)?;
    let ifd0 = t.ifd0()?;
    if let Some((e, typ, _)) = t.find(ifd0, ORIENTATION) {
        t.set_inline(e, typ, 1);
    }
    if let Some((e, _, _)) = t.find(ifd0, EXIF_IFD) {
        let sub = t.u32(e + 8)? as usize;
        for (tag, v) in [(PIXEL_X, p.width), (PIXEL_Y, p.height)] {
            if let Some((e, typ, _)) = t.find(sub, tag) {
                t.set_inline(e, typ, v);
            }
        }
    }
    // Drop IFD1 (the thumbnail): wipe its image bytes and unlink it.
    if let Some(next_at) = t.next_ifd_at(ifd0) {
        let ifd1 = t.u32(next_at).unwrap_or(0) as usize;
        if ifd1 != 0 {
            if let (Some((o, _, _)), Some((l, _, _))) = (t.find(ifd1, THUMB_OFFSET), t.find(ifd1, THUMB_LEN)) {
                let (off, len) = (t.u32(o + 8)? as usize, t.u32(l + 8)? as usize);
                t.zero(off, len);
            }
            t.put_u32(next_at, 0);
        }
    }
    if p.strip_location {
        if let Some((e, _, _)) = t.find(ifd0, GPS_IFD) {
            let gps = t.u32(e + 8)? as usize;
            // Wipe every value the GPS entries point to, then the entries themselves.
            for (entry, _, typ, count) in t.entries(gps) {
                let size = TYPE_SIZES.get(typ as usize).copied().unwrap_or(0) * count as usize;
                if size > 4 {
                    let off = t.u32(entry + 8)? as usize;
                    t.zero(off, size);
                }
            }
            let n = t.u16(gps)? as usize;
            t.zero(gps + 2, n * 12);
            t.put_u16(gps, 0);
        }
    }
    Some(data)
}

/// The profile's description ("Display P3", "sRGB IEC61966-2.1", …), from its 'desc' tag.
pub fn icc_description(icc: &[u8]) -> Option<String> {
    let be32 = |at: usize| icc.get(at..at + 4).map(|s| u32::from_be_bytes(s.try_into().unwrap()));
    let count = be32(128)? as usize;
    for i in 0..count.min(100) {
        let e = 132 + i * 12;
        if icc.get(e..e + 4)? != b"desc" {
            continue;
        }
        let (off, len) = (be32(e + 4)? as usize, be32(e + 8)? as usize);
        let tag = icc.get(off..off + len)?;
        return match tag.get(..4)? {
            b"desc" => {
                // v2: type, reserved, ASCII length, ASCII text
                let n = u32::from_be_bytes(tag.get(8..12)?.try_into().ok()?) as usize;
                let s = tag.get(12..12 + n)?;
                Some(String::from_utf8_lossy(s).trim_end_matches('\0').trim().to_owned())
            }
            b"mluc" => {
                // v4: first record's UTF-16BE string
                let rec_len = u32::from_be_bytes(tag.get(20..24)?.try_into().ok()?) as usize;
                let rec_off = u32::from_be_bytes(tag.get(24..28)?.try_into().ok()?) as usize;
                let s = tag.get(rec_off..rec_off + rec_len)?;
                let units: Vec<u16> = s.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                Some(String::from_utf16_lossy(&units).trim_end_matches('\0').trim().to_owned())
            }
            _ => None,
        };
    }
    None
}

/// IJG standard luminance quantization table (quality 50), in zigzag order like DQT stores it.
const STD_LUMA_ZIGZAG: [u16; 64] = [
    16, 11, 12, 14, 12, 10, 16, 14, 13, 14, 18, 17, 16, 19, 24, 40, 26, 24, 22, 22, 24, 49, 35, 37, 29, 40, 58, 51, 61,
    60, 57, 51, 56, 55, 64, 72, 92, 78, 64, 68, 87, 69, 55, 56, 80, 109, 81, 87, 95, 98, 103, 104, 103, 62, 77, 113,
    121, 112, 100, 120, 92, 101, 103, 99,
];

/// Estimates the quality setting (1–100, IJG scale used by almost every encoder) a JPEG was
/// saved at, from its luminance quantization table. Saving edits at the same quality keeps the
/// file size and look close to the original.
pub fn jpeg_quality(dqt_segments: &[&[u8]]) -> Option<u8> {
    for seg in dqt_segments {
        let mut i = 0;
        while i < seg.len() {
            let pq = seg[i] >> 4;
            let tq = seg[i] & 0x0f;
            let n = if pq == 0 { 64 } else { 128 };
            let body = seg.get(i + 1..i + 1 + n)?;
            if tq == 0 {
                let vals: Vec<u16> = if pq == 0 {
                    body.iter().map(|&v| v as u16).collect()
                } else {
                    body.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect()
                };
                let scale: f64 = vals.iter().zip(STD_LUMA_ZIGZAG).map(|(&q, s)| q as f64 * 100.0 / s as f64).sum::<f64>() / 64.0;
                let q = if scale <= 100.0 { (200.0 - scale) / 2.0 } else { 5000.0 / scale };
                return Some(q.round().clamp(1.0, 100.0) as u8);
            }
            i += 1 + n;
        }
    }
    None
}

/// Chroma subsampling of a JPEG from its SOF segment: (h, v) factors of the first component.
pub fn jpeg_sampling(sof: &[u8]) -> Option<(u8, u8)> {
    // precision(1) height(2) width(2) ncomp(1) then per component: id, HV, tq
    let ncomp = *sof.get(5)?;
    if ncomp < 3 {
        return Some((1, 1));
    }
    let hv = *sof.get(7)?;
    Some((hv >> 4, hv & 0x0f))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A little-endian EXIF block: IFD0 {Orientation=6, ExifIFD, GPS}, ExifIFD {PixelX, PixelY},
    /// GPS {LatitudeRef 'N', Latitude 3 rationals}, IFD1 {thumb offset/len}, thumbnail bytes.
    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; 400];
        b[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        let w16 = |b: &mut Vec<u8>, at: usize, v: u16| b[at..at + 2].copy_from_slice(&v.to_le_bytes());
        let w32 = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let entry = |b: &mut Vec<u8>, at: usize, tag: u16, typ: u16, count: u32, val: u32| {
            w16(b, at, tag);
            w16(b, at + 2, typ);
            w32(b, at + 4, count);
            w32(b, at + 8, val);
        };
        // IFD0 at 8: 3 entries, next IFD at 200
        w16(&mut b, 8, 3);
        entry(&mut b, 10, ORIENTATION, 3, 1, 6);
        entry(&mut b, 22, EXIF_IFD, 4, 1, 60);
        entry(&mut b, 34, GPS_IFD, 4, 1, 100);
        w32(&mut b, 46, 200);
        // Exif IFD at 60
        w16(&mut b, 60, 2);
        entry(&mut b, 62, PIXEL_X, 4, 1, 4032);
        entry(&mut b, 74, PIXEL_Y, 4, 1, 3024);
        // GPS IFD at 100: LatitudeRef inline, Latitude 3 rationals at 150
        w16(&mut b, 100, 2);
        entry(&mut b, 102, 1, 2, 2, u32::from_le_bytes([b'N', 0, 0, 0]));
        entry(&mut b, 114, 2, 5, 3, 150);
        for i in 0..24 {
            b[150 + i] = 0xAB;
        }
        // IFD1 at 200: thumbnail at 300, 50 bytes
        w16(&mut b, 200, 2);
        entry(&mut b, 202, THUMB_OFFSET, 4, 1, 300);
        entry(&mut b, 214, THUMB_LEN, 4, 1, 50);
        for i in 0..50 {
            b[300 + i] = 0xCD;
        }
        b
    }

    #[test]
    fn exif_is_made_upright_resized_and_thumbnail_free() {
        let out = patch_exif(&sample(), &ExifPatch { width: 1000, height: 800, strip_location: false }).unwrap();
        let mut o = out.clone();
        let t = Tiff::new(&mut o).unwrap();
        assert_eq!(t.u16(18), Some(1), "orientation");
        assert_eq!(t.u32(70), Some(1000));
        assert_eq!(t.u32(82), Some(800));
        assert_eq!(t.u32(46), Some(0), "IFD1 unlinked");
        assert!(out[300..350].iter().all(|&x| x == 0), "thumbnail wiped");
        assert!(out[150..174].iter().all(|&x| x == 0xAB), "location kept");
    }

    #[test]
    fn location_is_removed_completely() {
        let mut input = b"Exif\0\0".to_vec();
        input.extend(sample());
        let out = patch_exif(&input, &ExifPatch { width: 1, height: 1, strip_location: true }).unwrap();
        assert!(out[150..174].iter().all(|&x| x == 0), "coordinates wiped");
        assert_eq!(&out[100..102], &[0, 0], "GPS IFD empty");
        assert!(out[102..126].iter().all(|&x| x == 0));
    }

    #[test]
    fn quality_estimate_matches_ijg_scaling() {
        for q in [50u32, 75, 90, 95] {
            let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
            let mut seg = vec![0u8];
            seg.extend(STD_LUMA_ZIGZAG.iter().map(|&s| ((s as u32 * scale + 50) / 100).clamp(1, 255) as u8));
            let est = jpeg_quality(&[&seg]).unwrap() as i32;
            assert!((est - q as i32).abs() <= 1, "q{q} estimated {est}");
        }
    }

    #[test]
    fn icc_v2_description() {
        let mut icc = vec![0u8; 200];
        icc[128..132].copy_from_slice(&1u32.to_be_bytes());
        icc[132..136].copy_from_slice(b"desc");
        icc[136..140].copy_from_slice(&150u32.to_be_bytes());
        icc[140..144].copy_from_slice(&30u32.to_be_bytes());
        icc[150..154].copy_from_slice(b"desc");
        icc[158..162].copy_from_slice(&11u32.to_be_bytes());
        icc[162..173].copy_from_slice(b"Display P3\0");
        assert_eq!(icc_description(&icc).as_deref(), Some("Display P3"));
    }
}
