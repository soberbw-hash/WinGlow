//! Keep native minimum sizes inside the current monitor's usable desktop area.
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

fn minimum(work: PhysicalSize<u32>, scale: f64) -> PhysicalSize<u32> {
    PhysicalSize::new(
        ((760.0 * scale).ceil() as u32).min(work.width),
        ((600.0 * scale).ceil() as u32).min(work.height),
    )
}
fn constrain(
    origin: PhysicalPosition<i32>,
    work: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let size = PhysicalSize::new(size.width.min(work.width), size.height.min(work.height));
    let bound = |value: i32, start: i32, available: u32, used: u32| {
        i64::from(value).clamp(
            i64::from(start),
            i64::from(start) + i64::from(available - used),
        ) as i32
    };
    (
        PhysicalPosition::new(
            bound(position.x, origin.x, work.width, size.width),
            bound(position.y, origin.y, work.height, size.height),
        ),
        size,
    )
}
pub fn fit(window: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let work = monitor.work_area();
    if work.size.width == 0 || work.size.height == 0 {
        return Ok(());
    }
    window.set_min_size(Some(minimum(work.size, monitor.scale_factor())))?;
    // Maximized frame borders can lie outside the work area by design.
    if window.is_maximized()? || window.is_fullscreen()? {
        return Ok(());
    }
    let size = window.outer_size()?;
    let position = window.outer_position()?;
    let (next_position, next_size) = constrain(work.position, work.size, position, size);
    if next_size != size {
        window.set_size(next_size)?;
    }
    if next_position != position {
        window.set_position(next_position)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimum_and_initial_window_fit_2k_and_4k_with_taskbar_reserved() {
        for (width, height) in [(2560, 1440), (3840, 2160)] {
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0] {
                let work = PhysicalSize::new(width, height - (48.0_f64 * scale).ceil() as u32);
                let minimum = minimum(work, scale);
                assert!(minimum.width <= work.width && minimum.height <= work.height);
                let wanted = PhysicalSize::new((1080.0 * scale) as u32, (740.0 * scale) as u32);
                let (pos, size) = constrain(
                    PhysicalPosition::new(0, 0),
                    work,
                    PhysicalPosition::new(300, 200),
                    wanted,
                );
                assert!(pos.x >= 0 && pos.y >= 0);
                assert!(
                    pos.x as u32 + size.width <= work.width
                        && pos.y as u32 + size.height <= work.height
                );
            }
        }
    }
    #[test]
    fn secondary_monitors_keep_negative_origins_and_valid_user_positions() {
        let origin = PhysicalPosition::new(-2560, -1440);
        let work = PhysicalSize::new(2560, 1392);
        let position = PhysicalPosition::new(-2200, -1300);
        let size = PhysicalSize::new(1080, 740);
        assert_eq!(constrain(origin, work, position, size), (position, size));
        let (position, size) = constrain(origin, work, PhysicalPosition::new(-500, -500), size);
        assert_eq!(position, PhysicalPosition::new(-1080, -788));
        assert_eq!(size, PhysicalSize::new(1080, 740));
    }
}
