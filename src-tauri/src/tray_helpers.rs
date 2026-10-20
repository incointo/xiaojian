/// All inputs are physical pixels so mixed-DPI monitors share one coordinate system.
pub fn popup_position(
    pointer: (i32, i32),
    window_size: (u32, u32),
    work_area: (i32, i32, u32, u32),
) -> (i32, i32) {
    let (left, top, width, height) = work_area;
    let (window_width, window_height) = (window_size.0 as i32, window_size.1 as i32);
    let min_x = left + 12;
    let min_y = top + 12;
    let max_x = (left + width as i32 - window_width - 12).max(min_x);
    let max_y = (top + height as i32 - window_height - 12).max(min_y);
    (
        (pointer.0 - window_width + 20).clamp(min_x, max_x),
        (pointer.1 - window_height - 12).clamp(min_y, max_y),
    )
}

#[cfg(test)]
mod tests {
    use super::popup_position;

    #[test]
    fn popup_stays_above_bottom_taskbar() {
        assert_eq!(
            popup_position((1848, 1060), (340, 432), (0, 0, 1920, 1040)),
            (1528, 596)
        );
    }

    #[test]
    fn popup_handles_negative_monitor_origins_and_top_taskbar() {
        assert_eq!(
            popup_position((-1450, 1080), (360, 440), (-1920, 0, 1920, 1040)),
            (-1790, 588)
        );
        assert_eq!(
            popup_position((50, 5), (340, 432), (0, 40, 1920, 1040)),
            (12, 52)
        );
    }

    #[test]
    fn popup_handles_scaled_size_and_small_work_area() {
        assert_eq!(
            popup_position((3700, 2100), (510, 660), (1920, 0, 1920, 2080)),
            (3210, 1408)
        );
        assert_eq!(
            popup_position((50, 50), (340, 432), (0, 0, 200, 200)),
            (12, 12)
        );
    }
}
