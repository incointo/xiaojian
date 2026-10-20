pub const MIN_WIDTH: f64 = 340.0;

#[derive(Debug)]
pub struct HeightPlan {
    pub height: u32,
    pub min_height: u32,
    pub top: i32,
}

/// Content is measured in CSS/logical pixels; work areas and output use physical pixels.
pub fn main_height_plan(
    content_height: f64,
    scale: f64,
    work_top: i32,
    work_height: u32,
    current_top: i32,
) -> Option<HeightPlan> {
    if !content_height.is_finite()
        || content_height <= 0.0
        || !scale.is_finite()
        || scale <= 0.0
        || work_height == 0
    {
        return None;
    }
    let margin = ((12.0 * scale).round() as u32).min(work_height.saturating_sub(1) / 2);
    let available = work_height - margin * 2;
    let min_height = ((300.0 * scale).ceil() as u32).clamp(1, available);
    let max_height = ((720.0 * scale).ceil() as u32).clamp(min_height, available);
    let height = ((content_height * scale).ceil() as u32).clamp(min_height, max_height);
    let min_top = i64::from(work_top) + i64::from(margin);
    let max_top = i64::from(work_top) + i64::from(work_height - margin - height);
    let top = i64::from(current_top).clamp(min_top, max_top);
    Some(HeightPlan {
        height,
        min_height,
        top: top.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    })
}

#[cfg(test)]
mod tests {
    use super::main_height_plan;

    #[test]
    fn keeps_a_readable_minimum_and_clamps_to_work_area() {
        let plan = main_height_plan(100.0, 1.0, 0, 1040, 900).unwrap();
        assert_eq!(plan.height, 300);
        assert_eq!(plan.min_height, 300);
        assert_eq!(plan.top, 728);
        assert_eq!(
            main_height_plan(10_000.0, 1.0, 0, 1040, 100)
                .unwrap()
                .height,
            720
        );
    }

    #[test]
    fn converts_logical_content_height_using_current_dpi() {
        let plan = main_height_plan(500.0, 1.5, 0, 1040, 900).unwrap();
        assert_eq!(plan.height, 750);
        assert_eq!(plan.top, 272);
        assert_eq!(plan.min_height, 450);
        assert_eq!(
            main_height_plan(10_000.0, 1.5, 0, 1040, 0).unwrap().height,
            1004
        );
    }

    #[test]
    fn handles_negative_monitor_coordinates_and_short_scaled_screens() {
        let plan = main_height_plan(400.0, 1.0, -1080, 1040, -2000).unwrap();
        assert_eq!(plan.top, -1068);
        let short = main_height_plan(500.0, 2.0, 0, 540, 0).unwrap();
        assert_eq!(short.height, 492);
        assert_eq!(short.min_height, 492);
        assert!(short.top >= 0 && short.top + short.height as i32 <= 540);
    }

    #[test]
    fn rejects_invalid_measurements() {
        for height in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            assert!(main_height_plan(height, 1.0, 0, 1040, 0).is_none());
        }
        assert!(main_height_plan(400.0, 0.0, 0, 1040, 0).is_none());
        assert!(main_height_plan(400.0, 1.0, 0, 0, 0).is_none());
    }
}
