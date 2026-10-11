//! Native UI font settings complement name substitution; never replace system font files.
use crate::registry::{self, Entry, Scope, StoredValue};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use windows::Win32::{
    Graphics::Gdi::LOGFONTW,
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE, GetDpiForSystem,
            SetThreadDpiAwarenessContext,
        },
        WindowsAndMessaging::{
            NONCLIENTMETRICSW, SPI_GETICONTITLELOGFONT, SPI_GETNONCLIENTMETRICS,
            SPI_SETICONTITLELOGFONT, SPI_SETNONCLIENTMETRICS, SPIF_SENDCHANGE,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
    },
};

pub const NAMES: [&str; 6] = [
    "CaptionFont",
    "SmCaptionFont",
    "MenuFont",
    "StatusFont",
    "MessageFont",
    "IconFont",
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    dpi: u32,
    fonts: [Vec<u8>; 6],
}
struct DpiContext(DPI_AWARENESS_CONTEXT);
impl DpiContext {
    fn enter() -> Result<Self> {
        let old = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) };
        if old.0.is_null() {
            bail!("无法读取系统 DPI 上下文。");
        }
        Ok(Self(old))
    }
}
impl Drop for DpiContext {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
fn bytes(font: &LOGFONTW) -> Vec<u8> {
    // LOGFONTW has no padding: five i32s, eight u8s and 32 UTF-16 units.
    assert_eq!(std::mem::size_of::<LOGFONTW>(), 92);
    unsafe { std::slice::from_raw_parts((font as *const LOGFONTW).cast::<u8>(), 92) }.to_vec()
}
fn font(bytes: &[u8]) -> Result<LOGFONTW> {
    if bytes.len() != std::mem::size_of::<LOGFONTW>() {
        bail!("界面字体快照大小无效。");
    }
    let font: LOGFONTW = unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast()) };
    let end = font
        .lfFaceName
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| anyhow::anyhow!("界面字体名称无效。"))?;
    if end == 0
        || String::from_utf16(&font.lfFaceName[..end]).is_err()
        || !(-2048..=-1).contains(&font.lfHeight)
        || !(0..=2048).contains(&font.lfWidth)
        || !(0..=1000).contains(&font.lfWeight)
        || font.lfItalic > 1
        || font.lfUnderline > 1
        || font.lfStrikeOut > 1
        || font.lfEscapement.abs_diff(0) > 3600
        || font.lfOrientation.abs_diff(0) > 3600
    {
        bail!("界面字体快照参数无效。");
    }
    Ok(font)
}
fn decode(value: &StoredValue) -> Result<Snapshot> {
    if value.kind != 3 || value.bytes.len() > 8192 {
        bail!("界面字体快照类型无效。");
    }
    let snapshot: Snapshot = serde_json::from_slice(&value.bytes)?;
    if !(48..=1536).contains(&snapshot.dpi) {
        bail!("界面字体快照 DPI 无效。");
    }
    for bytes in &snapshot.fonts {
        font(bytes)?;
    }
    Ok(snapshot)
}
pub fn validate(value: Option<&StoredValue>) -> Result<()> {
    decode(value.ok_or_else(|| anyhow::anyhow!("界面字体实时快照不能缺失。"))?)?;
    Ok(())
}
fn encode(snapshot: &Snapshot) -> Result<StoredValue> {
    Ok(StoredValue {
        kind: 3,
        bytes: serde_json::to_vec(snapshot)?,
    })
}
fn metrics() -> Result<NONCLIENTMETRICSW> {
    let mut value = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            value.cbSize,
            Some((&mut value as *mut NONCLIENTMETRICSW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }?;
    Ok(value)
}
fn current() -> Result<Snapshot> {
    let _context = DpiContext::enter()?;
    let metrics = metrics()?;
    let mut icon = LOGFONTW::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            Some((&mut icon as *mut LOGFONTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }?;
    Ok(Snapshot {
        dpi: unsafe { GetDpiForSystem() },
        fonts: [
            metrics.lfCaptionFont,
            metrics.lfSmCaptionFont,
            metrics.lfMenuFont,
            metrics.lfStatusFont,
            metrics.lfMessageFont,
            icon,
        ]
        .map(|f| bytes(&f)),
    })
}
pub fn read() -> Result<StoredValue> {
    encode(&current()?)
}
fn scale(font: &mut LOGFONTW, from: u32, to: u32) {
    let convert = |n: i32| {
        ((i64::from(n) * i64::from(to)
            + if n < 0 {
                -i64::from(from) / 2
            } else {
                i64::from(from) / 2
            })
            / i64::from(from)) as i32
    };
    font.lfHeight = convert(font.lfHeight);
    font.lfWidth = convert(font.lfWidth);
}
fn fonts_at_dpi(snapshot: &Snapshot, dpi: u32) -> Result<Vec<LOGFONTW>> {
    let mut fonts = snapshot
        .fonts
        .iter()
        .map(|b| font(b))
        .collect::<Result<Vec<_>>>()?;
    for font in &mut fonts {
        // The system-aware SPI caller uses actual system-DPI pixels. A
        // DPI-unaware caller uses logical units; do not mix those contexts.
        scale(font, snapshot.dpi, dpi);
    }
    Ok(fonts)
}
pub(crate) fn matches_readback(actual: &StoredValue, expected: &StoredValue) -> Result<bool> {
    let actual = decode(actual)?;
    let mut expected = decode(expected)?;
    expected.fonts = fonts_at_dpi(&expected, actual.dpi)?
        .iter()
        .map(bytes)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| anyhow::anyhow!("界面字体角色数量无效。"))?;
    expected.dpi = actual.dpi;
    Ok(actual == expected)
}
pub fn write(value: &StoredValue) -> Result<()> {
    let snapshot = decode(value)?;
    let _context = DpiContext::enter()?;
    let mut fonts = fonts_at_dpi(&snapshot, unsafe { GetDpiForSystem() })?;
    // Read dimensions afresh: only the six fonts are changed, never menu/window sizes.
    let mut metrics = metrics()?;
    metrics.lfCaptionFont = fonts[0];
    metrics.lfSmCaptionFont = fonts[1];
    metrics.lfMenuFont = fonts[2];
    metrics.lfStatusFont = fonts[3];
    metrics.lfMessageFont = fonts[4];
    // Profile persistence is handled separately by journaled WindowMetrics entries.
    // No SPIF_UPDATEINIFILE: rollback preserves even originally absent registry values.
    unsafe {
        SystemParametersInfoW(
            SPI_SETNONCLIENTMETRICS,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            SPIF_SENDCHANGE,
        )
        .context("Windows 拒绝界面字体设置。")?;
        SystemParametersInfoW(
            SPI_SETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            Some((&mut fonts[5] as *mut LOGFONTW).cast()),
            SPIF_SENDCHANGE,
        )
        .context("Windows 拒绝桌面文字字体设置。")?;
    }
    // The shell's cached icon font is not always updated by the top-level
    // broadcast. Send the same setting notification directly to its view.
    let _ = crate::desktop::refresh_font();
    Ok(())
}
pub fn refresh() -> Result<()> {
    let expected = read()?;
    write(&expected)?;
    if !matches_readback(&read()?, &expected)? {
        bail!("刷新后界面字体与原设置不一致。");
    }
    verify_desktop_font()?;
    Ok(())
}
pub fn synchronize_desktop() -> Result<()> {
    // Desktop creation/reparenting can leave a stale cached font even when SPI
    // and the journalled profile are correct. Preserve the saved size and face.
    if crate::desktop::font_diagnostics().is_ok() && verify_desktop_font().is_err() {
        refresh()?;
    }
    Ok(())
}
pub fn verify_desktop_font() -> Result<()> {
    let snapshot = current()?;
    let desktop = crate::desktop::font_diagnostics()?;
    let dpi = desktop["dpi"].as_u64().context("桌面 DPI 无效。")? as u32;
    let mut expected = font(&snapshot.fonts[5])?;
    scale(&mut expected, snapshot.dpi, dpi);
    let end = expected
        .lfFaceName
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(32);
    let family = String::from_utf16_lossy(&expected.lfFaceName[..end]);
    if desktop["height"].as_i64() != Some(i64::from(expected.lfHeight))
        || desktop["weight"].as_i64() != Some(i64::from(expected.lfWeight))
        || desktop["family"].as_str() != Some(family.as_str())
    {
        bail!("字体已保存，但桌面仍显示旧字体。请稍后重试刷新。");
    }
    Ok(())
}
fn retarget(snapshot: &Snapshot, family: &str, weight: i32) -> Result<Snapshot> {
    let name: Vec<_> = family.encode_utf16().collect();
    if name.is_empty() || name.len() >= 32 || ![400, 700].contains(&weight) {
        bail!("界面字体目标无效。");
    }
    let mut desired = snapshot.clone();
    for stored in &mut desired.fonts {
        let mut value = font(stored)?;
        value.lfFaceName.fill(0);
        value.lfFaceName[..name.len()].copy_from_slice(&name);
        value.lfWeight = weight;
        *stored = bytes(&value);
    }
    Ok(desired)
}
fn entries(snapshot: &Snapshot, family: &str, heavier: bool) -> Result<Vec<Entry>> {
    let desired = retarget(snapshot, family, if heavier { 700 } else { 400 })?;
    snapshot_entries(&desired)
}
fn snapshot_entries(desired: &Snapshot) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for (name, stored) in NAMES.iter().zip(&desired.fonts) {
        // WindowMetrics stores font heights at the profile's actual DPI.
        // Normalizing to 96 shrinks 150% text from -18 to -12 after logon.
        let value = font(stored)?;
        entries.push(Entry {
            slot: registry::slot(Scope::WindowMetrics, *name),
            value: Some(StoredValue {
                kind: 3,
                bytes: bytes(&value),
            }),
        });
    }
    entries.push(Entry {
        slot: registry::slot(Scope::LiveUiFonts, "Fonts"),
        value: Some(encode(desired)?),
    });
    Ok(entries)
}
pub fn plan(family: &str, heavier: bool) -> Result<Vec<Entry>> {
    entries(&readable_snapshot(&current()?)?, family, heavier)
}
// One-click optimization must also repair tiny sizes left by older DPI bugs.
// Keep larger accessibility/custom sizes and never change Windows display scale.
fn readable_snapshot(snapshot: &Snapshot) -> Result<Snapshot> {
    let mut desired = snapshot.clone();
    let minimum = ((snapshot.dpi * 9 + 36) / 72) as i32;
    for stored in &mut desired.fonts {
        let mut value = font(stored)?;
        if -value.lfHeight < minimum {
            value.lfHeight = -minimum;
            // A width left by the same shrink must not constrain glyphs.
            value.lfWidth = 0;
            *stored = bytes(&value);
        }
    }
    Ok(desired)
}
pub fn optimization_plan(family: &str, heavier: bool) -> Result<Vec<Entry>> {
    entries(&readable_snapshot(&current()?)?, family, heavier)
}
pub fn readability_plan() -> Result<Vec<Entry>> {
    let original = current()?;
    let repaired = readable_snapshot(&original)?;
    if original == repaired {
        return Ok(Vec::new());
    }
    snapshot_entries(&repaired)
}
pub fn matches_current(family: &str, heavier: bool) -> Result<bool> {
    let snapshot = current()?;
    Ok(snapshot == retarget(&snapshot, family, if heavier { 700 } else { 400 })?)
}
pub fn current_message_font() -> Result<(String, u16)> {
    let snapshot = current()?;
    let value = font(&snapshot.fonts[4])?;
    let end = value.lfFaceName.iter().position(|c| *c == 0).unwrap_or(32);
    Ok((
        String::from_utf16_lossy(&value.lfFaceName[..end]),
        value.lfWeight as u16,
    ))
}
pub fn diagnostics() -> Result<serde_json::Value> {
    let snapshot = current()?;
    let fonts=NAMES.iter().zip(&snapshot.fonts).map(|(name,b)| {
        let font=font(b)?; let end=font.lfFaceName.iter().position(|c|*c==0).unwrap_or(32);
        Ok(serde_json::json!({"role":name,"family":String::from_utf16_lossy(&font.lfFaceName[..end]),"weight":font.lfWeight,"height":font.lfHeight}))
    }).collect::<Result<Vec<_>>>()?;
    let desktop = match crate::desktop::font_diagnostics() {
        Ok(value) => value,
        Err(error) => serde_json::json!({"error":format!("{error:#}")}),
    };
    Ok(serde_json::json!({"systemDpi":snapshot.dpi,"fonts":fonts,"desktop":desktop}))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        let mut value = LOGFONTW {
            lfHeight: -24,
            lfWeight: 400,
            lfQuality: windows::Win32::Graphics::Gdi::FONT_QUALITY(5),
            ..Default::default()
        };
        let name: Vec<_> = "Segoe UI".encode_utf16().collect();
        value.lfFaceName[..name.len()].copy_from_slice(&name);
        Snapshot {
            dpi: 192,
            fonts: std::array::from_fn(|_| bytes(&value)),
        }
    }
    #[test]
    fn optimization_repairs_legacy_tiny_fonts_at_every_scale_without_reducing_large_fonts() {
        for dpi in [96, 120, 144, 168, 192, 216, 240, 288] {
            let mut damaged = snapshot();
            damaged.dpi = dpi;
            for (index, stored) in damaged.fonts.iter_mut().enumerate() {
                let mut value = font(stored).unwrap();
                value.lfHeight = if index == 5 { -48 } else { -3 };
                *stored = bytes(&value);
            }
            let repaired = readable_snapshot(&damaged).unwrap();
            assert_eq!(
                font(&repaired.fonts[0]).unwrap().lfHeight,
                -(((dpi * 9 + 36) / 72) as i32)
            );
            assert_eq!(font(&repaired.fonts[5]).unwrap().lfHeight, -48);
            assert_eq!(readable_snapshot(&repaired).unwrap(), repaired);
            // The original remains available for exact rollback/undo.
            assert_eq!(font(&damaged.fonts[0]).unwrap().lfHeight, -3);
        }
        let mut source = snapshot();
        source.dpi = 144;
        source.fonts[5] = bytes(&LOGFONTW {
            lfHeight: -12,
            ..font(&source.fonts[5]).unwrap()
        });
        assert_eq!(
            font(&readable_snapshot(&source).unwrap().fonts[5])
                .unwrap()
                .lfHeight,
            -18
        );
    }
    #[test]
    fn two_weights_use_real_family_and_preserve_size_and_quality() {
        for weight in [400, 700] {
            let changed = retarget(&snapshot(), "HarmonyOS Sans SC", weight).unwrap();
            let f = font(&changed.fonts[2]).unwrap();
            assert_eq!(f.lfWeight, weight);
            assert_eq!(f.lfHeight, -24);
            assert_eq!(f.lfQuality.0, 5);
            assert_eq!(decode(&encode(&changed).unwrap()).unwrap(), changed);
        }
        let planned = entries(&snapshot(), "HarmonyOS Sans SC", true).unwrap();
        assert_eq!(
            font(&planned[2].value.as_ref().unwrap().bytes)
                .unwrap()
                .lfHeight,
            -24
        );
        let live = decode(planned[6].value.as_ref().unwrap()).unwrap();
        assert_eq!(live.dpi, 192);
        assert_eq!(font(&live.fonts[2]).unwrap().lfHeight, -24);
        assert_eq!(font(&live.fonts[2]).unwrap().lfWeight, 700);
    }
    #[test]
    fn snapshot_validation_rejects_missing_and_bad_data() {
        assert!(validate(None).is_err());
        assert!(
            validate(Some(&StoredValue {
                kind: 3,
                bytes: vec![0; 92]
            }))
            .is_err()
        );
        let mut invalid = snapshot();
        invalid.dpi = 0;
        assert!(decode(&encode(&invalid).unwrap()).is_err());
        invalid = snapshot();
        invalid.fonts[0].truncate(12);
        assert!(decode(&encode(&invalid).unwrap()).is_err());
    }
    #[test]
    fn readback_accepts_dpi_conversion_but_rejects_wrong_fonts_and_sizes() {
        let original = snapshot(); // 192 DPI, -24 pixels.
        let expected = encode(&original).unwrap();
        for dpi in [96, 120, 144, 168, 192, 240, 288] {
            let mut actual = original.clone();
            actual.dpi = dpi;
            actual.fonts = fonts_at_dpi(&original, dpi)
                .unwrap()
                .iter()
                .map(bytes)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            assert!(matches_readback(&encode(&actual).unwrap(), &expected).unwrap());
            let mut wrong = font(&actual.fonts[5]).unwrap();
            wrong.lfHeight -= 1;
            actual.fonts[5] = bytes(&wrong);
            assert!(!matches_readback(&encode(&actual).unwrap(), &expected).unwrap());
        }
        let wrong_family = retarget(&original, "Arial", 400).unwrap();
        assert!(!matches_readback(&encode(&wrong_family).unwrap(), &expected).unwrap());
        let wrong_weight = retarget(&original, "Segoe UI", 700).unwrap();
        assert!(!matches_readback(&encode(&wrong_weight).unwrap(), &expected).unwrap());
    }
    #[test]
    fn dpi_matrix_keeps_profile_sizes_and_uses_system_aware_spi_units() {
        // 100%, 125%, 150%, 175%, 200%, 225%, 250%, 300%.
        // Exercise distinct user sizes for all six roles, not just the default.
        for dpi in [96, 120, 144, 168, 192, 216, 240, 288] {
            let mut source = snapshot();
            source.dpi = dpi;
            for (index, stored) in source.fonts.iter_mut().enumerate() {
                let mut value = font(stored).unwrap();
                value.lfHeight = -(12 + index as i32 * 4);
                value.lfWidth = index as i32 * 4;
                scale(&mut value, 96, dpi);
                *stored = bytes(&value);
            }
            for heavier in [false, true] {
                let planned = entries(&source, "HarmonyOS Sans SC", heavier).unwrap();
                let live = decode(planned[6].value.as_ref().unwrap()).unwrap();
                let api_fonts = fonts_at_dpi(&live, dpi).unwrap();
                for index in 0..6 {
                    let original = font(&source.fonts[index]).unwrap();
                    let saved = font(&planned[index].value.as_ref().unwrap().bytes).unwrap();
                    assert_eq!(saved.lfHeight, original.lfHeight, "profile DPI {dpi}");
                    assert_eq!(saved.lfWidth, original.lfWidth);
                    assert_eq!(
                        api_fonts[index].lfHeight, original.lfHeight,
                        "SPI DPI {dpi}"
                    );
                    assert_eq!(api_fonts[index].lfWidth, original.lfWidth);
                }
                // Reapplying a plan must not shrink fonts on each optimization.
                assert_eq!(
                    entries(&live, "HarmonyOS Sans SC", heavier).unwrap(),
                    planned
                );
                // Restoring an old snapshot uses its recorded DPI, even after
                // moving to a display with a different scale.
                let restored =
                    fonts_at_dpi(&decode(&encode(&source).unwrap()).unwrap(), 96).unwrap();
                assert_eq!(restored[5].lfHeight, -32);
            }
        }
    }
    #[test]
    fn native_read_is_only_read_and_round_trips() {
        let before = read().unwrap();
        validate(Some(&before)).unwrap();
        assert_eq!(before, read().unwrap());
        assert_eq!(decode(&before).unwrap().fonts.len(), 6);
    }
    #[test]
    #[ignore = "Explicit real-desktop acceptance: writes then restores live UI fonts"]
    fn native_write_round_trips_without_repeated_shrinking() {
        let _guard = crate::worker::optimization_lock().unwrap();
        let original = read().unwrap();
        let result = (|| -> Result<()> {
            let changed = encode(&retarget(&decode(&original)?, "Arial", 700)?)?;
            write(&changed)?;
            if read()? != changed {
                bail!("Native font family and weight change did not round-trip");
            }
            for _ in 0..3 {
                write(&original)?;
                if read()? != original {
                    bail!("System-aware native font write did not round-trip");
                }
            }
            Ok(())
        })();
        write(&original).unwrap();
        result.unwrap();
        assert_eq!(read().unwrap(), original);
    }
}
