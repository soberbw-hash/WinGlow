//! Extract resource icons without instantiating or executing shell extensions.
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
};
use windows::{
    Win32::{
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, HGDIOBJ, SelectObject,
        },
        System::Environment::ExpandEnvironmentStringsW,
        UI::{
            Shell::ExtractIconExW,
            WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON},
        },
    },
    core::PCWSTR,
};

pub fn location(raw: &str) -> Option<(String, i32)> {
    let raw = raw.trim().trim_start_matches('@');
    let (path, index) = raw
        .rsplit_once(',')
        .and_then(|(path, index)| index.trim().parse::<i32>().ok().map(|index| (path, index)))
        .unwrap_or((raw, 0));
    let path = path.trim().trim_matches('"');
    if path.is_empty() || path.contains('\0') {
        return None;
    }
    let wide: Vec<_> = path.encode_utf16().chain([0]).collect();
    let mut expanded = vec![0u16; 32768];
    let count =
        unsafe { ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), Some(&mut expanded)) } as usize;
    if count == 0 || count > expanded.len() {
        return None;
    }
    let expanded = String::from_utf16_lossy(&expanded[..count - 1]);
    // Resource reads must be local and absolute; do not touch network shares.
    if expanded.as_bytes().get(1) != Some(&b':') || !Path::new(&expanded).is_absolute() {
        return None;
    }
    let extension = Path::new(&expanded)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "exe" | "dll" | "ico" | "icl") {
        return None;
    }
    Some((expanded, index))
}

pub fn extract(raw: &str) -> Option<String> {
    let (path, index) = location(raw)?;
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
        return None;
    }
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = format!(
        "{path}:{index}:{:?}:{}",
        metadata.modified().ok(),
        metadata.len()
    );
    let mut cache = cache.lock().ok()?;
    if let Some(icon) = cache.get(&key) {
        return icon.clone();
    }
    let icon = render(&path, index);
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(key, icon.clone());
    icon
}

fn render(path: &str, index: i32) -> Option<String> {
    let wide: Vec<_> = path.encode_utf16().chain([0]).collect();
    let mut icon = HICON::default();
    let count = unsafe { ExtractIconExW(PCWSTR(wide.as_ptr()), index, Some(&mut icon), None, 1) };
    if count != 1 || icon.is_invalid() {
        return None;
    }
    // Draw on a white menu surface, then encode as a bounded 32x32 PNG.
    let result = unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            let _ = DestroyIcon(icon);
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(_) => {
                let _ = DeleteDC(dc);
                let _ = DestroyIcon(icon);
                return None;
            }
        };
        let old = SelectObject(dc, HGDIOBJ(bitmap.0));
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), 32 * 32 * 4);
        pixels.fill(255);
        let drawn = DrawIconEx(dc, 0, 0, icon, 32, 32, 0, None, DI_NORMAL).is_ok();
        let mut rgb = Vec::with_capacity(32 * 32 * 3);
        for pixel in pixels.chunks_exact(4) {
            rgb.extend([pixel[2], pixel[1], pixel[0]]);
        }
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = DestroyIcon(icon);
        drawn.then_some(rgb)
    }?;
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 32, 32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .ok()?
        .write_image_data(&result)
        .ok()?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icon_locations_preserve_resource_indices_and_reject_network_paths() {
        assert_eq!(
            location("\"C:\\Program Files\\App\\app.exe\",-12"),
            Some(("C:\\Program Files\\App\\app.exe".into(), -12))
        );
        assert!(location("\\\\server\\share\\app.dll,0").is_none());
        assert!(location("app.exe,0").is_none());
        assert!(location("C:\\test\\file.txt,0").is_none());
    }
    #[test]
    fn windows_resource_icon_is_a_real_png() {
        let icon = extract("%SystemRoot%\\System32\\shell32.dll,0").unwrap();
        let bytes = STANDARD
            .decode(icon.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
