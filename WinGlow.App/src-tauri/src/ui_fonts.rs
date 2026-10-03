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
            SPI_SETICONTITLELOGFONT, SPI_SETNONCLIENTMETRICS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
            SystemParametersInfoW,
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
pub fn write(value: &StoredValue) -> Result<()> {
    let snapshot = decode(value)?;
    let _context = DpiContext::enter()?;
    let dpi = unsafe { GetDpiForSystem() };
    let mut fonts = snapshot
        .fonts
        .iter()
        .map(|b| font(b))
        .collect::<Result<Vec<_>>>()?;
    for font in &mut fonts {
        scale(font, snapshot.dpi, dpi);
    }
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
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .context("Windows 拒绝界面字体设置。")?;
        SystemParametersInfoW(
            SPI_SETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            Some((&mut fonts[5] as *mut LOGFONTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .context("Windows 拒绝桌面文字字体设置。")?;
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
    let mut entries = Vec::new();
    for (name, stored) in NAMES.iter().zip(&desired.fonts) {
        let mut value = font(stored)?;
        scale(&mut value, desired.dpi, 96);
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
        value: Some(encode(&desired)?),
    });
    Ok(entries)
}
pub fn plan(family: &str, heavier: bool) -> Result<Vec<Entry>> {
    entries(&current()?, family, heavier)
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
    Ok(serde_json::json!({"systemDpi":snapshot.dpi,"fonts":fonts}))
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
            -12
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
    fn native_read_is_only_read_and_round_trips() {
        let before = read().unwrap();
        validate(Some(&before)).unwrap();
        assert_eq!(before, read().unwrap());
        assert_eq!(decode(&before).unwrap().fonts.len(), 6);
    }
}
