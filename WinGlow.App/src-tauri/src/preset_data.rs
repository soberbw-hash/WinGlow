use crate::models::FontPreset;
use anyhow::{Result, bail};

pub const MANAGED_ALIASES: &[&str] = &[
    "Segoe UI",
    "Segoe UI Light",
    "Segoe UI Semilight",
    "Segoe UI Semibold",
    "Segoe UI Bold",
    "Segoe UI Black",
    "Segoe UI Variable",
    "Segoe UI Variable Text",
    "Segoe UI Variable Text Light",
    "Segoe UI Variable Text Semibold",
    "Segoe UI Variable Display",
    "Segoe UI Variable Display Light",
    "Segoe UI Variable Display Semibold",
    "Segoe UI Variable Small",
    "Segoe UI Variable Small Light",
    "Segoe UI Variable Small Semibold",
    "Microsoft YaHei",
    "Microsoft YaHei UI",
    "Microsoft YaHei Light",
    "Microsoft YaHei UI Light",
    "Microsoft YaHei Semibold",
    "Microsoft YaHei UI Semibold",
    "Microsoft YaHei Bold",
    "Microsoft YaHei UI Bold",
];

#[derive(Clone, Copy)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub family: &'static str,
    pub preview: &'static str,
    pub bundled: bool,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "harmonyos-sc",
        label: "HarmonyOS Sans",
        family: "HarmonyOS Sans SC",
        preview: "HarmonyOS Preview",
        bundled: true,
    },
    Preset {
        id: "source-han-sans-cn",
        label: "思源黑体",
        family: "Source Han Sans CN",
        preview: "Source Han Preview",
        bundled: true,
    },
    Preset {
        id: "pingfang-sc",
        label: "苹方",
        family: "PingFang SC",
        preview: "PingFang Preview",
        bundled: true,
    },
];

pub fn find_preset(id: &str) -> Result<&'static Preset> {
    PRESETS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| anyhow::anyhow!("未知字体方案。"))
}

pub fn api_preset(preset: &Preset, available: bool) -> FontPreset {
    FontPreset {
        id: preset.id.into(),
        label: preset.label.into(),
        preview_family: format!("\"{}\", \"Microsoft YaHei UI\", sans-serif", preset.preview),
        available,
    }
}

pub fn validate_family(preset: &Preset, family: &str) -> Result<()> {
    // Font collections may include TC/HK faces; only the chosen SC family is registered.
    if family != preset.family {
        bail!(
            "字体内部名称不匹配：需要 {}，收到 {family}。",
            preset.family
        );
    }
    Ok(())
}

pub fn target_for_alias<'a>(
    alias: &str,
    regular: &'a str,
    medium: &'a str,
    bold: &'a str,
) -> &'a str {
    if alias.contains("Bold") || alias.ends_with("Black") {
        bold
    } else if alias.contains("Semibold") {
        medium
    } else {
        regular
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weight_mapping_preserves_regular_and_bold() {
        assert_eq!(target_for_alias("Microsoft YaHei", "R", "M", "B"), "R");
        assert_eq!(target_for_alias("Segoe UI Semibold", "R", "M", "B"), "M");
        assert_eq!(
            target_for_alias("Microsoft YaHei UI Bold", "R", "M", "B"),
            "B"
        );
        assert!(
            !MANAGED_ALIASES
                .iter()
                .any(|s| s.contains("Emoji") || s.contains("Symbol") || s.contains("MDL2"))
        );
    }
    #[test]
    fn selection_has_three_known_presets() {
        assert_eq!(PRESETS.len(), 3);
        assert!(find_preset("vivo").is_err());
        assert!(validate_family(find_preset("pingfang-sc").unwrap(), "Microsoft YaHei").is_err());
    }
}
