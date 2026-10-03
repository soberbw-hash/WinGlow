use crate::{
    models::{ActionResult, BootstrapPayload, Operation},
    preset_data::{self, MANAGED_ALIASES, PRESETS, Preset},
    registry::{self, Entry, Scope, WindowsRegistry},
    transaction::{self, ValueStore},
};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use ttf_parser::{Face, name_id};
use windows::{
    Win32::{
        Foundation::{LPARAM, WPARAM},
        Graphics::Gdi::{AddFontResourceExW, FONT_RESOURCE_CHARACTERISTICS, RemoveFontResourceExW},
        UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_FONTCHANGE, WM_SETTINGCHANGE,
        },
    },
    core::PCWSTR,
};

pub fn data_root() -> Result<PathBuf> {
    // Compatibility storage: do not strand old snapshots, imported fonts or live Breeze paths.
    Ok(dirs::data_local_dir()
        .ok_or_else(|| anyhow!("无法定位当前用户的数据目录。"))?
        .join("WindowsFontTuner"))
}
pub fn backup_root() -> Result<PathBuf> {
    Ok(data_root()?.join("Backups"))
}
pub fn new_backup_dir() -> Result<PathBuf> {
    Ok(backup_root()?.join(format!(
        "font-v3-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.9fZ"),
        uuid::Uuid::new_v4()
    )))
}
fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain([0]).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FontRecord {
    family: String,
    full_name: String,
    weight: u16,
    kind: String,
    path: PathBuf,
}
impl FontRecord {
    fn registry_name(&self) -> String {
        format!("{} ({})", self.full_name, self.kind)
    }
}

fn font_name(face: &Face, id: u16) -> Option<String> {
    let names = face.names();
    names
        .into_iter()
        .filter(|n| n.name_id == id && n.language_id == 0x0409)
        .chain(names.into_iter().filter(|n| n.name_id == id))
        .filter_map(|n| n.to_string())
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
}

fn inspect(bytes: &[u8], extension: &str, preset: &Preset) -> Result<Vec<FontRecord>> {
    if bytes.len() > 64_000_000 {
        bail!("单个字体文件不能超过 64 MB。");
    }
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    if count > 64 {
        bail!("字体集合包含过多字面。");
    }
    let mut records = Vec::new();
    for index in 0..count {
        let face = Face::parse(bytes, index).map_err(|_| anyhow!("字体文件损坏或不受支持。"))?;
        let family = font_name(&face, name_id::TYPOGRAPHIC_FAMILY)
            .or_else(|| font_name(&face, name_id::FAMILY))
            .ok_or_else(|| anyhow!("字体缺少内部字体族名称。"))?;
        if count > 1 && family != preset.family {
            continue;
        }
        preset_data::validate_family(preset, &family)?;
        let full_name =
            font_name(&face, name_id::FULL_NAME).ok_or_else(|| anyhow!("字体缺少完整名称。"))?;
        let kind = if matches!(extension, "otf" | "otc") {
            "OpenType"
        } else {
            "TrueType"
        };
        registry::validate_slot(&registry::slot(
            Scope::Fonts,
            format!("{full_name} ({kind})"),
        ))?;
        if face.is_variable() {
            bail!("请导入静态字重版本，避免旧式 Windows 界面无法正确选择字重。");
        }
        if face.is_italic() {
            continue;
        }
        for c in "中文设置文件0123456789AaBbGg，。！？：；（）".chars() {
            if face.glyph_index(c).is_none() {
                bail!("字体 {full_name} 缺少常用字符「{c}」，未应用。");
            }
        }
        let em = i32::from(face.units_per_em());
        if em < 16
            || face.ascender() <= 0
            || face.descender() > 0
            || i32::from(face.ascender()) - i32::from(face.descender()) > em * 4
        {
            bail!("字体行高信息异常，未应用。");
        }
        records.push(FontRecord {
            family,
            full_name,
            weight: face.weight().to_number(),
            kind: kind.into(),
            path: PathBuf::new(),
        });
    }
    if records.is_empty() {
        bail!("没有找到 {} 的简体中文字面。", preset.family);
    }
    Ok(records)
}

fn font_cache() -> Result<PathBuf> {
    let root =
        std::env::var_os("ProgramData").ok_or_else(|| anyhow!("无法定位系统字体缓存目录。"))?;
    Ok(PathBuf::from(root).join("WinGlow").join("Fonts"))
}

fn stage(bytes: &[u8], extension: &str, preset: &Preset) -> Result<Vec<FontRecord>> {
    let mut records = inspect(bytes, extension, preset)?;
    let digest = format!("{:x}", Sha256::digest(bytes));
    let root = font_cache()?;
    fs::create_dir_all(&root).context("创建字体资源目录失败。")?;
    let path = root.join(format!("{digest}.{extension}"));
    if !path.exists() {
        transaction::persist_new(&path, bytes).context("保存字体资源失败。")?;
    }
    if Sha256::digest(&fs::read(&path)?) != Sha256::digest(bytes) {
        bail!("字体资源校验失败。");
    }
    for record in &mut records {
        record.path = path.clone();
    }
    Ok(records)
}

fn bundled_records(preset: &Preset) -> Result<Vec<FontRecord>> {
    let files: &[(&[u8], &str)] = match preset.id {
        "harmonyos-sc" => &[
            (
                include_bytes!("../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Regular.ttf"),
                "ttf",
            ),
            (
                include_bytes!("../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Medium.ttf"),
                "ttf",
            ),
            (
                include_bytes!("../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Bold.ttf"),
                "ttf",
            ),
        ],
        "source-han-sans-cn" => &[
            (
                include_bytes!(
                    "../../../FontPackages/source-han-sans-cn/SourceHanSansCN-Regular.otf"
                ),
                "otf",
            ),
            (
                include_bytes!(
                    "../../../FontPackages/source-han-sans-cn/SourceHanSansCN-Medium.otf"
                ),
                "otf",
            ),
            (
                include_bytes!("../../../FontPackages/source-han-sans-cn/SourceHanSansCN-Bold.otf"),
                "otf",
            ),
        ],
        _ => bail!("此字体需要自行导入。"),
    };
    let mut records = Vec::new();
    for (bytes, ext) in files {
        records.extend(stage(bytes, ext, preset)?);
    }
    Ok(records)
}

fn imported_records(paths: &[String]) -> Result<Vec<FontRecord>> {
    if paths.is_empty() || paths.len() > 16 {
        bail!("请选择 1 到 16 个苹方字体文件，包含常规与粗体。");
    }
    let preset = preset_data::find_preset("pingfang-sc")?;
    let mut records = Vec::new();
    for input in paths {
        let path = Path::new(input);
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            bail!("只支持 TTF、OTF、TTC、OTC 字体文件。");
        }
        if !path.is_absolute()
            || !fs::metadata(path)?.is_file()
            || fs::metadata(path)?.len() > 64_000_000
        {
            bail!("字体文件路径或大小无效。");
        }
        records.extend(stage(&fs::read(path)?, &ext, preset)?);
    }
    // Duplicate faces can make GDI select an unpredictable file.
    let mut names = std::collections::BTreeSet::new();
    for r in &records {
        if !names.insert(r.registry_name()) {
            bail!("选择了重复的字体字面：{}。", r.full_name);
        }
    }
    weight_faces(&records)?;
    Ok(records)
}

fn weight_faces(records: &[FontRecord]) -> Result<(&FontRecord, &FontRecord, &FontRecord)> {
    let regular = records
        .iter()
        .filter(|r| (350..=450).contains(&r.weight))
        .min_by_key(|r| r.weight.abs_diff(400))
        .ok_or_else(|| anyhow!("缺少常规字重（Regular）。"))?;
    let bold = records
        .iter()
        .filter(|r| (600..=800).contains(&r.weight))
        .min_by_key(|r| r.weight.abs_diff(700))
        .ok_or_else(|| anyhow!("缺少粗体字重（Semibold 或 Bold），未应用。"))?;
    let medium = records
        .iter()
        .filter(|r| (500..=600).contains(&r.weight))
        .min_by_key(|r| r.weight.abs_diff(500))
        .unwrap_or(bold);
    Ok((regular, medium, bold))
}

fn saved_pingfang() -> Result<Vec<FontRecord>> {
    let path = data_root()?.join("pingfang-v3.json");
    let records: Vec<FontRecord> =
        serde_json::from_slice(&fs::read(path).context("请先导入苹方字体。")?)?;
    let root = font_cache()?;
    let preset = preset_data::find_preset("pingfang-sc")?;
    for r in &records {
        if r.path.parent() != Some(root.as_path()) {
            bail!("字体记录路径无效。");
        }
        preset_data::validate_family(preset, &r.family)?;
        let ext = r.path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let bytes = fs::read(&r.path).context("已导入字体资源缺失，请重新导入。")?;
        let name = r.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if name != format!("{:x}", Sha256::digest(&bytes)) {
            bail!("已导入字体校验失败，请重新导入。");
        }
        if !inspect(&bytes, ext, preset)?.iter().any(|face| {
            face.full_name == r.full_name && face.weight == r.weight && face.kind == r.kind
        }) {
            bail!("字体记录与文件不一致。");
        }
    }
    weight_faces(&records)?;
    Ok(records)
}

fn font_entries(records: &[FontRecord]) -> Vec<Entry> {
    records
        .iter()
        .map(|r| Entry {
            slot: registry::slot(Scope::Fonts, r.registry_name()),
            value: Some(registry::string(&r.path.to_string_lossy())),
        })
        .collect()
}

fn plan(preset: &Preset, records: &[FontRecord]) -> Result<Vec<Entry>> {
    let (regular, medium, bold) = weight_faces(records)?;
    let mut entries = font_entries(records);
    for alias in MANAGED_ALIASES {
        entries.push(Entry {
            slot: registry::slot(Scope::Substitutes, *alias),
            value: Some(registry::string(preset_data::target_for_alias(
                alias,
                &regular.full_name,
                &medium.full_name,
                &bold.full_name,
            ))),
        });
    }
    // Link the actual destination family, preserve existing Windows base-family links.
    // No self links, no emoji/symbol substitutions, no changes to ClearType or WPF rendering.
    let windows = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
        .join("Fonts");
    let mut fallback = Vec::new();
    for (file, family) in [
        ("seguiemj.ttf", "Segoe UI Emoji"),
        ("simsun.ttc", "SimSun"),
        ("msjh.ttc", "Microsoft JhengHei"),
    ] {
        if windows.join(file).is_file() {
            fallback.push(format!("{file},{family}"));
        }
    }
    if fallback.is_empty() {
        bail!("系统回退字体缺失，未应用。");
    }
    let names: std::collections::BTreeSet<_> = records
        .iter()
        .map(|r| r.full_name.clone())
        .chain(std::iter::once(preset.family.into()))
        .collect();
    for name in names {
        entries.push(Entry {
            slot: registry::slot(Scope::Links, name),
            value: Some(registry::multi_string(&fallback)),
        });
    }
    Ok(entries)
}

fn activate(records: &[FontRecord]) -> Result<Vec<PathBuf>> {
    let paths: std::collections::BTreeSet<_> = records.iter().map(|r| r.path.clone()).collect();
    let mut added = Vec::new();
    for path in paths {
        let s = wide(&path);
        let count = unsafe {
            AddFontResourceExW(PCWSTR(s.as_ptr()), FONT_RESOURCE_CHARACTERISTICS(0), None)
        };
        if count == 0 {
            deactivate(&added);
            bail!("Windows 拒绝载入字体：{}。", path.display());
        }
        added.push(path);
    }
    Ok(added)
}
fn deactivate(paths: &[PathBuf]) {
    for p in paths {
        let s = wide(p);
        unsafe {
            let _ = RemoveFontResourceExW(PCWSTR(s.as_ptr()), 0, None);
        }
    }
}
pub(crate) fn notify() {
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_FONTCHANGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
        let s = wide("FontSubstitutes");
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(s.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}

fn installed_pingfang() -> bool {
    saved_pingfang().is_ok_and(|records| {
        font_entries(&records)
            .iter()
            .all(|e| WindowsRegistry.read(&e.slot).is_ok_and(|v| v == e.value))
    })
}

pub fn load_bootstrap() -> Result<BootstrapPayload> {
    let store = WindowsRegistry;
    let read = |name| -> Result<Option<String>> {
        Ok(store
            .read(&registry::slot(Scope::Substitutes, name))?
            .and_then(|v| registry::as_string(&v)))
    };
    let cn = read("Microsoft YaHei UI")?
        .or(read("Microsoft YaHei")?)
        .unwrap_or_else(|| "Microsoft YaHei UI".into());
    let en = read("Segoe UI")?.unwrap_or_else(|| "Segoe UI".into());
    let aliases = MANAGED_ALIASES
        .iter()
        .map(|name| Ok((*name, read(*name)?)))
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    let active = PRESETS.iter().find(|p| {
        let names = if p.bundled {
            [
                p.family.to_string(),
                format!("{} Medium", p.family),
                format!("{} Bold", p.family),
            ]
        } else {
            let Ok(records) = saved_pingfang() else {
                return false;
            };
            let Ok((regular, medium, bold)) = weight_faces(&records) else {
                return false;
            };
            [
                regular.full_name.clone(),
                medium.full_name.clone(),
                bold.full_name.clone(),
            ]
        };
        MANAGED_ALIASES.iter().all(|name| {
            aliases.get(name).and_then(|value| value.as_deref())
                == Some(preset_data::target_for_alias(
                    name, &names[0], &names[1], &names[2],
                ))
        })
    });
    let label = active.map(|p| p.label.to_string()).unwrap_or_else(|| {
        if cn == "Microsoft YaHei UI" && en == "Segoe UI" {
            "Windows 默认".into()
        } else {
            format!("{en} / {cn}")
        }
    });
    let family = if let Some(p) = active {
        format!("\"{}\", \"Microsoft YaHei UI\", sans-serif", p.preview)
    } else {
        format!(
            "{}, {}, sans-serif",
            serde_json::to_string(&en)?,
            serde_json::to_string(&cn)?
        )
    };
    let root = backup_root()?;
    fs::create_dir_all(&root)?;
    let dirs = transaction::backup_directories(&root)?;
    let pending = dirs.iter().find(|d| transaction::is_pending(d));
    let damaged = pending.is_some_and(|d| transaction::read_journal(d).is_err());
    let pingfang = installed_pingfang();
    Ok(BootstrapPayload {
        presets: PRESETS
            .iter()
            .map(|p| preset_data::api_preset(p, p.bundled || pingfang))
            .collect(),
        active_preset_id: active.map(|p| p.id.into()),
        active_font_label: label,
        current_preview_family: family,
        backup_dir: root.to_string_lossy().into(),
        can_restore: dirs
            .iter()
            .any(|d| transaction::is_pending(d) || d.join("committed").exists())
            || legacy_backup(false)?.is_some(),
        pending_recovery: pending.map(|_| {
            if damaged {
                "备份无法读取。请到还原页扫描修复。"
            } else {
                "上次修改没有完成，请先还原。"
            }
            .into()
        }),
    })
}

pub fn perform(operation: Operation) -> Result<ActionResult> {
    let mut store = WindowsRegistry;
    let dirs = transaction::backup_directories(&backup_root()?)?;
    if !matches!(&operation,Operation::Restore{mode} if mode=="last" || mode=="force-default")
        && dirs.iter().any(|d| transaction::is_pending(d))
    {
        bail!("存在未完成的修改，请先恢复。");
    }
    match operation {
        Operation::Apply { preset_id, paths } => {
            if !paths.is_empty() {
                bail!("请先导入并预览字体，再点击应用。");
            }
            let preset = preset_data::find_preset(&preset_id)?;
            let records = if preset.bundled {
                bundled_records(preset)?
            } else {
                saved_pingfang()?
            };
            let desired = plan(preset, &records)?;
            let mut added = Vec::new();
            let result = transaction::execute(
                &mut store,
                &new_backup_dir()?,
                &format!("apply:{preset_id}"),
                desired,
                || {
                    added = activate(&records)?;
                    Ok(())
                },
            );
            if result.is_err() {
                deactivate(&added);
            }
            notify();
            result?;
            Ok(ActionResult {
                message: "已应用。重新打开应用查看效果；部分界面需要注销后生效。".into(),
            })
        }
        Operation::Import { paths } => {
            let records = imported_records(&paths)?;
            let mut added = Vec::new();
            let result = transaction::execute(
                &mut store,
                &new_backup_dir()?,
                "import:pingfang-sc",
                font_entries(&records),
                || {
                    added = activate(&records)?;
                    Ok(())
                },
            );
            if result.is_err() {
                deactivate(&added);
            }
            notify();
            result?;
            fs::create_dir_all(data_root()?)?;
            let tmp = data_root()?.join(format!("pingfang-{}.tmp", uuid::Uuid::new_v4()));
            transaction::persist_new(&tmp, &serde_json::to_vec_pretty(&records)?)?;
            // MoveFileExW atomically replaces the metadata after the registry transaction commits.
            crate::worker::replace_file(&tmp, &data_root()?.join("pingfang-v3.json"))?;
            Ok(ActionResult {
                message: "已导入苹方。查看更换后预览，再点击应用。".into(),
            })
        }
        Operation::Restore { mode } => {
            let force = mode == "force-default";
            let source = dirs
                .iter()
                .find(|d| transaction::is_pending(d))
                .or_else(|| {
                    dirs.iter().find(|d| {
                        d.join("committed").exists()
                            && !d.join("undone").exists()
                            && transaction::read_journal(d)
                                .is_ok_and(|j| j.operation.starts_with("apply:"))
                    })
                });
            let desired = if mode == "last" {
                if let Some(dir) = source {
                    transaction::read_journal(dir)?.before
                } else {
                    legacy_restore()?.ok_or_else(|| anyhow!("没有可恢复的备份。"))?
                }
            } else if mode == "default" || force {
                let mut entries = Vec::new();
                for alias in MANAGED_ALIASES {
                    entries.push(Entry {
                        slot: registry::slot(Scope::Substitutes, *alias),
                        value: None,
                    });
                }
                // Earliest snapshots contain the original link values. Load each journal once.
                let mut originals = std::collections::BTreeMap::new();
                for dir in dirs.iter().rev() {
                    let journal = match transaction::read_journal(dir) {
                        Ok(j) => j,
                        Err(_) if force => continue,
                        Err(e) => return Err(e),
                    };
                    if !(dir.join("committed").exists() || transaction::is_pending(dir)) {
                        continue;
                    }
                    for entry in journal.before {
                        if matches!(
                            entry.slot.scope,
                            Scope::Links | Scope::Desktop | Scope::Avalon(_)
                        ) {
                            originals.entry(entry.slot.clone()).or_insert(entry);
                        }
                    }
                }
                entries.extend(originals.into_values());
                if let Some(dir) = legacy_backup(true)? {
                    match crate::legacy::read_backup(&dir) {
                        Ok(old) => entries.extend(
                            old.into_iter()
                                .filter(|e| e.slot.scope != Scope::Substitutes),
                        ),
                        Err(_) if force => {}
                        Err(e) => return Err(e),
                    };
                }
                // v2 recovery can already have copied alias links into a v3 journal.
                // Prefer the original v2 link value and write each slot exactly once.
                unique_entries(entries)
            } else {
                bail!("未知恢复方式。");
            };
            let backup = transaction::execute(
                &mut store,
                &new_backup_dir()?,
                &format!("restore:{mode}"),
                desired,
                || Ok(()),
            );
            notify();
            backup?;
            if let Some(dir) = source.filter(|d| !force && transaction::is_pending(d)) {
                transaction::persist_new(&dir.join("recovered"), b"ok")?;
            } else if mode == "last"
                && let Some(dir) = source
            {
                transaction::persist_new(&dir.join("undone"), b"ok")?;
            }
            if force {
                for dir in &dirs {
                    if transaction::is_pending(dir)
                        && transaction::read_journal(dir).map_or(true, |j| {
                            j.before.iter().all(|e| {
                                matches!(
                                    e.slot.scope,
                                    Scope::Substitutes
                                        | Scope::Links
                                        | Scope::Fonts
                                        | Scope::SystemFonts
                                        | Scope::UserSystemFonts
                                        | Scope::Desktop
                                        | Scope::Avalon(_)
                                )
                            })
                        })
                    {
                        // Preserve the original snapshot; an explicit repair is not an exact undo.
                        transaction::persist_new(
                            &dir.join("quarantined"),
                            b"Explicit system font repair; exact snapshot recovery not completed.",
                        )?;
                    }
                }
            }
            Ok(ActionResult {
                message: "已恢复。重新打开应用查看效果；部分界面需要注销后生效。".into(),
            })
        }
        _ => bail!("此操作不属于字体设置。"),
    }
}

fn legacy_backup(first: bool) -> Result<Option<PathBuf>> {
    let root = backup_root()?;
    let mut dirs = fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_dir())
                && e.file_name().to_string_lossy().starts_with("backup-")
        })
        .map(|e| e.path())
        .filter(|p| p.join("FontSubstitutes.reg").is_file())
        .collect::<Vec<_>>();
    dirs.sort();
    Ok(if first {
        dirs.into_iter().next()
    } else {
        dirs.pop()
    })
}

fn legacy_restore() -> Result<Option<Vec<Entry>>> {
    legacy_backup(false)?
        .map(|dir| crate::legacy::read_backup(&dir))
        .transpose()
}

fn unique_entries(entries: Vec<Entry>) -> Vec<Entry> {
    entries
        .into_iter()
        .map(|entry| (entry.slot.clone(), entry))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_default_restore_writes_overlapping_link_once() {
        let slot = registry::slot(Scope::Links, "Segoe UI");
        let entries = unique_entries(vec![
            Entry {
                slot: slot.clone(),
                value: Some(registry::multi_string(&["v3".into()])),
            },
            Entry { slot, value: None },
        ]);
        registry::validate_entries(&entries).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value, None);
    }
    #[test]
    fn bundled_fonts_have_real_regular_medium_bold_and_common_glyphs() {
        for (id, files, ext) in [
            (
                "harmonyos-sc",
                vec![
                    include_bytes!(
                        "../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Regular.ttf"
                    )
                    .as_slice(),
                    include_bytes!(
                        "../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Medium.ttf"
                    )
                    .as_slice(),
                    include_bytes!("../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Bold.ttf")
                        .as_slice(),
                ],
                "ttf",
            ),
            (
                "source-han-sans-cn",
                vec![
                    include_bytes!(
                        "../../../FontPackages/source-han-sans-cn/SourceHanSansCN-Regular.otf"
                    )
                    .as_slice(),
                    include_bytes!(
                        "../../../FontPackages/source-han-sans-cn/SourceHanSansCN-Medium.otf"
                    )
                    .as_slice(),
                    include_bytes!(
                        "../../../FontPackages/source-han-sans-cn/SourceHanSansCN-Bold.otf"
                    )
                    .as_slice(),
                ],
                "otf",
            ),
        ] {
            let p = preset_data::find_preset(id).unwrap();
            let mut records = Vec::new();
            for bytes in files {
                records.extend(inspect(bytes, ext, p).unwrap());
            }
            let (r, m, b) = weight_faces(&records).unwrap();
            assert_eq!(r.weight, 400);
            assert_eq!(m.weight, 500);
            assert_eq!(b.weight, 700);
        }
    }
    #[test]
    fn rejects_invalid_and_misidentified_fonts() {
        assert!(
            inspect(
                b"not a font",
                "ttf",
                preset_data::find_preset("pingfang-sc").unwrap()
            )
            .is_err()
        );
        assert!(
            inspect(
                include_bytes!("../../../FontPackages/harmonyos-sc/HarmonyOS_Sans_SC_Regular.ttf"),
                "ttf",
                preset_data::find_preset("pingfang-sc").unwrap()
            )
            .is_err()
        );
    }
}
