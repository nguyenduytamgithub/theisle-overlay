use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityPreset {
    Balanced,
    Clear,
    Ultra,
}

impl Default for VisibilityPreset {
    fn default() -> Self {
        Self::Ultra
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityRenderer {
    #[default]
    None,
    GpuAdaptive,
    MagnifierFallback,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RendererReadback {
    pub(crate) renderer: VisibilityRenderer,
    pub(crate) game_hwnd: isize,
    pub(crate) source: (i32, i32, i32, i32),
    pub(crate) preset: VisibilityPreset,
    pub(crate) presented_frames: u64,
    pub(crate) last_presented_at: Instant,
    pub(crate) median_interval_ms: f32,
    pub(crate) scene_luma: f32,
}

impl RendererReadback {
    pub(crate) fn is_current_for(
        &self,
        renderer: VisibilityRenderer,
        game_hwnd: isize,
        source: (i32, i32, i32, i32),
        preset: VisibilityPreset,
        now: Instant,
    ) -> bool {
        self.renderer == renderer
            && self.game_hwnd == game_hwnd
            && self.source == source
            && self.preset == preset
            && self.presented_frames > 0
            && self.median_interval_ms.is_finite()
            && self.median_interval_ms > 0.0
            && self.scene_luma.is_finite()
            && (0.0..=1.0).contains(&self.scene_luma)
            && now
                .checked_duration_since(self.last_presented_at)
                .is_some_and(|age| age <= Duration::from_millis(500))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LuminanceController {
    smoothed: f32,
    last_sample_at: Option<Instant>,
}

impl LuminanceController {
    pub(crate) fn new(initial: f32) -> Self {
        let smoothed = if initial.is_finite() {
            initial.clamp(0.005, 1.0)
        } else {
            0.18
        };
        Self {
            smoothed,
            last_sample_at: None,
        }
    }

    pub(crate) fn sample_at(&mut self, measured: f32, now: Instant) -> Option<f32> {
        if self.last_sample_at.is_some_and(|last| {
            now.checked_duration_since(last)
                .is_none_or(|elapsed| elapsed < Duration::from_millis(250))
        }) {
            return None;
        }

        let measured = if measured.is_finite() {
            measured.clamp(0.005, 1.0)
        } else {
            self.smoothed
        };
        let alpha = if measured < self.smoothed { 0.72 } else { 0.18 };
        self.smoothed = (self.smoothed + (measured - self.smoothed) * alpha)
            .clamp(measured.min(self.smoothed), measured.max(self.smoothed));
        self.last_sample_at = Some(now);
        Some(self.smoothed)
    }

    pub(crate) fn smoothed(&self) -> f32 {
        self.smoothed
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub(crate) struct VisibilityParameters {
    pub(crate) exposure: f32,
    pub(crate) shadow_lift: f32,
    pub(crate) gamma: f32,
    pub(crate) highlight_knee: f32,
    pub(crate) saturation: f32,
    pub(crate) detail_gain: f32,
}

#[derive(Clone, Copy)]
struct PresetLimits {
    target_luma: f32,
    max_exposure: f32,
    max_shadow_lift: f32,
    min_gamma: f32,
    highlight_knee: f32,
    saturation: f32,
    detail_gain: f32,
}

impl VisibilityPreset {
    fn limits(self) -> PresetLimits {
        match self {
            Self::Balanced => PresetLimits {
                target_luma: 0.18,
                max_exposure: 3.2,
                max_shadow_lift: 0.025,
                min_gamma: 0.78,
                highlight_knee: 0.80,
                saturation: 1.04,
                detail_gain: 0.25,
            },
            Self::Clear => PresetLimits {
                target_luma: 0.26,
                max_exposure: 5.0,
                max_shadow_lift: 0.070,
                min_gamma: 0.62,
                highlight_knee: 0.73,
                saturation: 1.01,
                detail_gain: 0.60,
            },
            Self::Ultra => PresetLimits {
                target_luma: 0.36,
                max_exposure: 8.0,
                max_shadow_lift: 0.120,
                min_gamma: 0.48,
                highlight_knee: 0.64,
                saturation: 0.96,
                detail_gain: 1.05,
            },
        }
    }
}

fn finite_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(crate) fn preset_parameters(
    preset: VisibilityPreset,
    strength: u8,
    scene_luma: f32,
) -> VisibilityParameters {
    let limits = preset.limits();
    let amount = f32::from(strength.min(100)) / 100.0;
    let scene_luma = if scene_luma.is_finite() {
        scene_luma.clamp(0.005, 1.0)
    } else {
        0.18
    };
    let requested_exposure = (limits.target_luma / scene_luma).clamp(1.0, limits.max_exposure);

    VisibilityParameters {
        exposure: 1.0 + (requested_exposure - 1.0) * amount,
        shadow_lift: limits.max_shadow_lift * amount,
        gamma: 1.0 - (1.0 - limits.min_gamma) * amount,
        highlight_knee: 0.98 + (limits.highlight_knee - 0.98) * amount,
        saturation: 1.0 + (limits.saturation - 1.0) * amount,
        detail_gain: limits.detail_gain * amount,
    }
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let unit = ((value - low) / (high - low)).clamp(0.0, 1.0);
    unit * unit * (3.0 - 2.0 * unit)
}

fn compress_highlight(value: f32, knee: f32) -> f32 {
    if value <= knee {
        return value;
    }
    let room = (1.0 - knee).max(0.001);
    knee + room * (1.0 - (-(value - knee) / room).exp())
}

fn rgb_luma(rgb: [f32; 3]) -> f32 {
    rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}

pub(crate) fn transform_rgb(
    rgb: [f32; 3],
    local_average: [f32; 3],
    parameters: VisibilityParameters,
) -> [f32; 3] {
    let rgb = rgb.map(finite_unit);
    let local_average = local_average.map(finite_unit);
    let source_luma = rgb_luma(rgb);
    let shadow_weight = 1.0 - smoothstep(0.30, 0.88, source_luma);

    let mut mapped = rgb.map(|channel| {
        let exposed = (channel * parameters.exposure).clamp(0.0, 1.0);
        let curved = exposed.powf(parameters.gamma);
        let lifted = curved + parameters.shadow_lift * (1.0 - curved) * shadow_weight;
        compress_highlight(lifted, parameters.highlight_knee)
    });

    let mapped_luma = rgb_luma(mapped);
    for channel in &mut mapped {
        *channel = mapped_luma + (*channel - mapped_luma) * parameters.saturation;
    }

    for index in 0..3 {
        let detail = (rgb[index] - local_average[index]) * parameters.detail_gain * shadow_weight;
        mapped[index] = (mapped[index] + detail).clamp(0.0, 1.0);
    }
    mapped
}

#[cfg(test)]
mod tests {
    use super::{
        preset_parameters, transform_rgb, LuminanceController, RendererReadback,
        VisibilityParameters, VisibilityPreset, VisibilityRenderer,
    };
    use serde::Deserialize;
    use std::time::{Duration, Instant};

    #[derive(Clone, Debug, Deserialize)]
    struct FixturePixel {
        rgb: [f32; 3],
        local: [f32; 3],
    }

    fn fixtures(source: &str) -> Vec<FixturePixel> {
        serde_json::from_str(source).expect("visibility fixture must be valid JSON")
    }

    fn luma(rgb: [f32; 3]) -> f32 {
        rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
    }

    fn assert_parameters_are_bounded(parameters: VisibilityParameters) {
        for value in [
            parameters.exposure,
            parameters.shadow_lift,
            parameters.gamma,
            parameters.highlight_knee,
            parameters.saturation,
            parameters.detail_gain,
        ] {
            assert!(value.is_finite(), "parameter is not finite: {value}");
        }
        assert!((1.0..=8.0).contains(&parameters.exposure));
        assert!((0.0..=0.30).contains(&parameters.shadow_lift));
        assert!((0.35..=1.0).contains(&parameters.gamma));
        assert!((0.55..=0.98).contains(&parameters.highlight_knee));
        assert!((0.65..=1.35).contains(&parameters.saturation));
        assert!((0.0..=1.5).contains(&parameters.detail_gain));
    }

    #[test]
    fn every_preset_strength_and_scene_luma_is_finite_and_bounded() {
        for preset in [
            VisibilityPreset::Balanced,
            VisibilityPreset::Clear,
            VisibilityPreset::Ultra,
        ] {
            for strength in 0..=u8::MAX {
                for scene_luma in [f32::NAN, f32::NEG_INFINITY, -1.0, 0.0, 0.04, 0.5, 1.0, 2.0] {
                    assert_parameters_are_bounded(preset_parameters(preset, strength, scene_luma));
                }
            }
        }
    }

    #[test]
    fn dark_scene_strength_orders_balanced_clear_and_ultra() {
        let balanced = preset_parameters(VisibilityPreset::Balanced, 85, 0.03);
        let clear = preset_parameters(VisibilityPreset::Clear, 85, 0.03);
        let ultra = preset_parameters(VisibilityPreset::Ultra, 85, 0.03);

        assert!(balanced.exposure < clear.exposure);
        assert!(clear.exposure < ultra.exposure);
        assert!(balanced.shadow_lift < clear.shadow_lift);
        assert!(clear.shadow_lift < ultra.shadow_lift);
        assert!(balanced.detail_gain < clear.detail_gain);
        assert!(clear.detail_gain < ultra.detail_gain);
    }

    #[test]
    fn transform_is_bounded_deterministic_and_lifts_black_without_whiteout() {
        let parameters = preset_parameters(VisibilityPreset::Ultra, 85, 0.02);
        let black = transform_rgb([0.0; 3], [0.0; 3], parameters);
        assert!(black.iter().all(|channel| (0.03..=0.30).contains(channel)));

        let near_white = transform_rgb([0.95, 0.94, 0.93], [0.94; 3], parameters);
        assert!(near_white
            .iter()
            .all(|channel| (0.0..1.0).contains(channel)));
        assert!(near_white.iter().any(|channel| *channel < 0.995));

        for rgb in [
            [f32::NAN, f32::INFINITY, f32::NEG_INFINITY],
            [-2.0, 0.5, 4.0],
            [0.01, 0.04, 0.09],
        ] {
            let first = transform_rgb(rgb, [0.03; 3], parameters);
            let second = transform_rgb(rgb, [0.03; 3], parameters);
            assert_eq!(first, second);
            assert!(first.iter().all(|channel| channel.is_finite()));
            assert!(first.iter().all(|channel| (0.0..=1.0).contains(channel)));
        }
    }

    #[test]
    fn ultra_increases_dark_local_separation() {
        let parameters = preset_parameters(VisibilityPreset::Ultra, 85, 0.025);
        let lower = transform_rgb([0.012; 3], [0.022; 3], parameters);
        let upper = transform_rgb([0.032; 3], [0.022; 3], parameters);
        assert!(luma(upper) - luma(lower) > 0.020);
    }

    #[test]
    fn fixture_gain_prioritizes_dark_content_over_bright_content() {
        let dark = fixtures(include_str!("../../tests/fixtures/visibility-dark.json"));
        let mixed = fixtures(include_str!("../../tests/fixtures/visibility-mixed.json"));
        let bright = fixtures(include_str!("../../tests/fixtures/visibility-bright.json"));
        let parameters = preset_parameters(VisibilityPreset::Ultra, 85, 0.03);

        let mean_gain = |pixels: &[FixturePixel]| {
            pixels
                .iter()
                .map(|pixel| {
                    luma(transform_rgb(pixel.rgb, pixel.local, parameters)) - luma(pixel.rgb)
                })
                .sum::<f32>()
                / pixels.len() as f32
        };
        let mean_ratio = |pixels: &[FixturePixel]| {
            pixels
                .iter()
                .map(|pixel| {
                    luma(transform_rgb(pixel.rgb, pixel.local, parameters))
                        / luma(pixel.rgb).max(0.001)
                })
                .sum::<f32>()
                / pixels.len() as f32
        };

        let dark_gain = mean_gain(&dark);
        let mixed_gain = mean_gain(&mixed);
        let bright_gain = mean_gain(&bright);
        let dark_ratio = mean_ratio(&dark);
        let mixed_ratio = mean_ratio(&mixed);
        let bright_ratio = mean_ratio(&bright);
        assert!(dark_gain > 0.12, "dark fixture gain was {dark_gain}");
        assert!(dark_ratio > mixed_ratio);
        assert!(mixed_ratio > bright_ratio);
        assert!(mixed_gain > bright_gain);
        assert!(
            bright_gain < 0.12,
            "bright fixture changed by {bright_gain}"
        );
    }

    #[test]
    fn luminance_sampling_is_rate_limited_and_sanitized() {
        let start = Instant::now();
        let mut controller = LuminanceController::new(0.18);

        assert!(controller.sample_at(0.03, start).is_some());
        assert!(controller
            .sample_at(0.01, start + Duration::from_millis(249))
            .is_none());
        assert!(controller
            .sample_at(0.01, start + Duration::from_millis(250))
            .is_some());
        assert!(controller
            .sample_at(f32::NAN, start + Duration::from_millis(500))
            .is_some());
        assert!(controller.smoothed().is_finite());
        assert!((0.005..=1.0).contains(&controller.smoothed()));
    }

    #[test]
    fn darkness_arrives_faster_than_brightness_exits_without_overshoot() {
        let start = Instant::now();
        let mut darkening = LuminanceController::new(0.50);
        let dark = darkening.sample_at(0.02, start).unwrap();
        assert!((0.02..0.20).contains(&dark), "dark response was {dark}");

        let mut brightening = LuminanceController::new(0.02);
        let bright = brightening.sample_at(0.50, start).unwrap();
        assert!(
            (0.02..0.15).contains(&bright),
            "bright exit response was {bright}"
        );
        assert!(darkening.smoothed() >= 0.02 && darkening.smoothed() <= 0.50);
        assert!(brightening.smoothed() >= 0.02 && brightening.smoothed() <= 0.50);
    }

    fn readback(now: Instant) -> RendererReadback {
        RendererReadback {
            renderer: VisibilityRenderer::GpuAdaptive,
            game_hwnd: 101,
            source: (100, 200, 1920, 1080),
            preset: VisibilityPreset::Ultra,
            presented_frames: 120,
            last_presented_at: now - Duration::from_millis(100),
            median_interval_ms: 16.7,
            scene_luma: 0.03,
        }
    }

    #[test]
    fn renderer_readback_requires_current_target_preset_and_recent_frame() {
        let now = Instant::now();
        let current = readback(now);
        assert!(current.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            101,
            (100, 200, 1920, 1080),
            VisibilityPreset::Ultra,
            now,
        ));

        let mut stale = current;
        stale.last_presented_at = now - Duration::from_millis(501);
        assert!(!stale.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            101,
            stale.source,
            VisibilityPreset::Ultra,
            now,
        ));
        assert!(!current.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            999,
            current.source,
            VisibilityPreset::Ultra,
            now,
        ));
        assert!(!current.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            101,
            (100, 200, 1280, 720),
            VisibilityPreset::Ultra,
            now,
        ));
        assert!(!current.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            101,
            current.source,
            VisibilityPreset::Clear,
            now,
        ));

        let mut empty = current;
        empty.presented_frames = 0;
        assert!(!empty.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            101,
            empty.source,
            VisibilityPreset::Ultra,
            now,
        ));
    }
}
