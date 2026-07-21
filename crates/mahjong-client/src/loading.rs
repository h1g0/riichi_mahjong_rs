//! Startup loading screen shared by the native and WASM clients.

use macroquad::prelude::*;

const SPINNER_DOTS: usize = 8;
const SPINNER_STEP_SECONDS: f64 = 0.1;

/// Animated loading screen used while startup work proceeds cooperatively.
pub struct LoadingScreen {
    completed_steps: usize,
    total_steps: usize,
    web_overlay_hidden: bool,
}

impl LoadingScreen {
    pub fn new(total_steps: usize) -> Self {
        assert!(total_steps > 0);
        Self {
            completed_steps: 0,
            total_steps,
            web_overlay_hidden: false,
        }
    }

    /// Presents the current animation frame and yields to the platform event loop.
    pub async fn next_frame(&mut self) {
        draw(
            get_time(),
            progress_ratio(self.completed_steps, self.total_steps),
        );
        next_frame().await;

        if !self.web_overlay_hidden {
            hide_web_overlay();
            self.web_overlay_hidden = true;
        }
    }

    /// Records one completed startup unit before presenting the next frame.
    pub async fn complete_step(&mut self) {
        debug_assert!(self.completed_steps < self.total_steps);
        self.completed_steps = (self.completed_steps + 1).min(self.total_steps);
        self.next_frame().await;
    }
}

fn draw(now: f64, progress: f32) {
    clear_background(Color::from_rgba(6, 14, 9, 255));
    crate::renderer::set_design_camera();

    let label = "Loading...";
    let font_size = 28;
    let dimensions = measure_text(label, None, font_size, 1.0);
    let spinner_diameter = 28.0;
    let gap = 18.0;
    let group_width = spinner_diameter + gap + dimensions.width;
    let group_x = (crate::renderer::DESIGN_W - group_width) / 2.0;
    let center_y = crate::renderer::DESIGN_H / 2.0;
    let spinner_center = vec2(group_x + spinner_diameter / 2.0, center_y);
    let head = spinner_head(now);

    for index in 0..SPINNER_DOTS {
        let angle = index as f32 * std::f32::consts::TAU / SPINNER_DOTS as f32;
        let distance_from_head = (head + SPINNER_DOTS - index) % SPINNER_DOTS;
        let alpha = 1.0 - distance_from_head as f32 * 0.1;
        draw_circle(
            spinner_center.x + angle.cos() * 11.0,
            spinner_center.y + angle.sin() * 11.0,
            3.0,
            Color::new(0.91, 0.78, 0.29, alpha),
        );
    }

    draw_text_ex(
        label,
        group_x + spinner_diameter + gap,
        center_y + dimensions.height / 2.0,
        TextParams {
            font_size,
            color: Color::from_rgba(236, 228, 210, 255),
            ..Default::default()
        },
    );

    let bar_width = 320.0;
    let bar_height = 10.0;
    let bar_x = (crate::renderer::DESIGN_W - bar_width) / 2.0;
    let bar_y = center_y + 42.0;
    draw_rectangle(
        bar_x,
        bar_y,
        bar_width,
        bar_height,
        Color::from_rgba(5, 14, 8, 255),
    );
    draw_rectangle(
        bar_x,
        bar_y,
        bar_width * progress,
        bar_height,
        Color::from_rgba(201, 162, 39, 255),
    );
    draw_rectangle_lines(
        bar_x,
        bar_y,
        bar_width,
        bar_height,
        1.0,
        Color::from_rgba(232, 200, 74, 150),
    );

    let percentage = format!("{:.0}%", progress * 100.0);
    let percentage_size = 18;
    let percentage_dimensions = measure_text(&percentage, None, percentage_size, 1.0);
    draw_text_ex(
        &percentage,
        (crate::renderer::DESIGN_W - percentage_dimensions.width) / 2.0,
        bar_y + bar_height + percentage_dimensions.height + 10.0,
        TextParams {
            font_size: percentage_size,
            color: Color::from_rgba(163, 188, 171, 255),
            ..Default::default()
        },
    );
}

fn spinner_head(now: f64) -> usize {
    ((now / SPINNER_STEP_SECONDS) as usize) % SPINNER_DOTS
}

fn progress_ratio(completed_steps: usize, total_steps: usize) -> f32 {
    debug_assert!(total_steps > 0);
    completed_steps.min(total_steps) as f32 / total_steps as f32
}

#[cfg(target_arch = "wasm32")]
fn hide_web_overlay() {
    // loading.js registers this import before the WASM module is instantiated.
    unsafe { mahjong_loading_hide() };
}

#[cfg(not(target_arch = "wasm32"))]
fn hide_web_overlay() {}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn mahjong_loading_hide();
}

/// Version handshake for the JavaScript loading-overlay plugin.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mahjong_loading_crate_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_advances_and_wraps() {
        assert_eq!(spinner_head(0.0), 0);
        assert_eq!(spinner_head(SPINNER_STEP_SECONDS), 1);
        assert_eq!(spinner_head(SPINNER_STEP_SECONDS * SPINNER_DOTS as f64), 0);
    }

    #[test]
    fn progress_ratio_tracks_steps_and_stays_bounded() {
        assert_eq!(progress_ratio(0, 42), 0.0);
        assert_eq!(progress_ratio(21, 42), 0.5);
        assert_eq!(progress_ratio(42, 42), 1.0);
        assert_eq!(progress_ratio(43, 42), 1.0);
    }
}
