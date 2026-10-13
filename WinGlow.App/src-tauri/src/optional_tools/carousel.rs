//! Legacy playlist cleanup. Automatic switching has been removed because the
//! upstream player closes the old wallpaper before showing the next one.
use super::*;
pub(super) fn stop() -> Result<()> {
    match fs::remove_file(lively_file("WinGlowCarousel.json")?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn active() -> bool {
    false
}
pub(super) fn start_host() -> Result<()> {
    host()
}
pub(super) fn host() -> Result<()> {
    // Existing login startup commands retain compatibility, but create no timer.
    stop()?;
    ensure_lively_started(&lively_executable()?)
}
pub(super) fn first_project() -> Result<PathBuf> {
    let cards = wallpaper_library()?.items;
    library_project(&default_card(&cards)?.id)
}
fn default_card(cards: &[WallpaperCard]) -> Result<&WallpaperCard> {
    cards
        .iter()
        .find(|item| item.title.eq_ignore_ascii_case("Rain"))
        .context("默认壁纸 Rain 不在壁纸库中，请重新安装壁纸组件。")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rain_is_default_regardless_of_library_order() {
        let cards = ["Fluids", "Rain", "The Hill"].map(|title| WallpaperCard {
            id: format!("library:{title}"),
            title: title.into(),
            preview: None,
            scene: None,
        });
        assert_eq!(default_card(&cards).unwrap().id, "library:Rain");
        assert!(default_card(&cards[..1]).is_err());
        assert!(default_card(&[]).is_err());
    }
    #[test]
    fn automatic_rotation_cannot_reactivate_from_an_old_backup() {
        assert!(!super::active());
    }
}
