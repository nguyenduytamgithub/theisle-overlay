use std::ffi::c_void;
use std::fmt;
use std::sync::OnceLock;

use super::visibility::VisibilityPreset;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Magnification::{
    MagGetColorEffect, MagGetWindowSource, MagInitialize, MagSetColorEffect,
    MagSetWindowFilterList, MagSetWindowSource, MagSetWindowTransform, MAGCOLOREFFECT,
    MAGTRANSFORM, MW_FILTERMODE_EXCLUDE, WC_MAGNIFIER,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, FindWindowExW, IsWindow, IsWindowVisible, KillTimer, SetTimer,
    SetWindowPos, SET_WINDOW_POS_FLAGS, WS_CHILD, WS_EX_TRANSPARENT,
};

const CHILD_TITLE: windows::core::PCWSTR = windows::core::w!("Night Boost Magnifier");
const POSITION_FLAGS: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0010 | 0x0004);
const SHOW_FLAGS: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0010 | 0x0004 | 0x0040);
const SOURCE_REFRESH_TIMER_ID: usize = 1;
const SOURCE_REFRESH_INTERVAL_MS: u32 = 16;
static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MagnifierProfile {
    pub(crate) gain: f32,
    pub(crate) black_translation: f32,
    pub(crate) cross_channel_luma: f32,
}

pub(crate) fn fallback_profile(preset: VisibilityPreset, strength: u8) -> MagnifierProfile {
    let amount = f32::from(strength.min(100)) / 100.0;
    let (gain_range, black_translation, cross_channel_luma) = match preset {
        VisibilityPreset::Balanced => (3.0, 0.010, 0.020),
        VisibilityPreset::Clear => (4.0, 0.030, 0.040),
        VisibilityPreset::Ultra => (5.0, 0.060, 0.060),
    };
    MagnifierProfile {
        gain: 1.0 + gain_range * amount,
        black_translation: black_translation * amount,
        cross_channel_luma: cross_channel_luma * amount,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MagnifierReadback {
    pub(crate) host: isize,
    pub(crate) source: (i32, i32, i32, i32),
    pub(crate) gain: f32,
    pub(crate) profile: MagnifierProfile,
    pub(crate) child: isize,
    pub(crate) excluded: Vec<isize>,
    pub(crate) refresh_interval_ms: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MagnifierError {
    operation: &'static str,
    detail: String,
}

impl fmt::Display for MagnifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed: {}", self.operation, self.detail)
    }
}

impl std::error::Error for MagnifierError {}

pub(crate) fn configure(
    host: isize,
    source: (i32, i32, i32, i32),
    profile: MagnifierProfile,
    excluded: &[isize],
) -> Result<MagnifierReadback, MagnifierError> {
    let (_, _, width, height) = source;
    if host == 0
        || width <= 0
        || height <= 0
        || !profile.gain.is_finite()
        || !(1.0..=6.0).contains(&profile.gain)
        || !profile.black_translation.is_finite()
        || !(0.0..=0.08).contains(&profile.black_translation)
        || !profile.cross_channel_luma.is_finite()
        || !(0.0..=0.08).contains(&profile.cross_channel_luma)
    {
        return Err(MagnifierError {
            operation: "validate magnifier configuration",
            detail: format!("host={host} source={source:?} profile={profile:?}"),
        });
    }
    ensure_initialized()?;
    destroy(host)?;

    let host = hwnd(host);
    let child = unsafe {
        CreateWindowExW(
            WS_EX_TRANSPARENT,
            WC_MAGNIFIER,
            CHILD_TITLE,
            WS_CHILD,
            0,
            0,
            width,
            height,
            Some(host),
            None,
            None,
            None,
        )
    }
    .map_err(|error| MagnifierError {
        operation: "CreateWindowExW(Magnifier)",
        detail: error.to_string(),
    })?;

    let configured = configure_child(child, host, source, profile, excluded);
    if configured.is_err() {
        let _ = unsafe { DestroyWindow(child) };
    }
    configured
}

fn configure_child(
    child: HWND,
    host: HWND,
    source: (i32, i32, i32, i32),
    profile: MagnifierProfile,
    excluded: &[isize],
) -> Result<MagnifierReadback, MagnifierError> {
    let (_, _, width, height) = source;

    unsafe { SetWindowPos(child, None, 0, 0, width, height, POSITION_FLAGS) }.map_err(|error| {
        MagnifierError {
            operation: "SetWindowPos(Magnifier)",
            detail: error.to_string(),
        }
    })?;

    let mut transform = identity_transform();
    bool_result(
        "MagSetWindowTransform",
        unsafe { MagSetWindowTransform(child, &mut transform) }.as_bool(),
    )?;

    let mut effect = color_effect(profile);
    bool_result(
        "MagSetColorEffect",
        unsafe { MagSetColorEffect(child, &mut effect) }.as_bool(),
    )?;

    let excluded_raw = normalized_exclusions(host.0 as isize, excluded);
    let mut excluded_windows: Vec<HWND> = excluded_raw.iter().copied().map(hwnd).collect();
    bool_result(
        "MagSetWindowFilterList",
        unsafe {
            MagSetWindowFilterList(
                child,
                MW_FILTERMODE_EXCLUDE,
                excluded_windows.len() as i32,
                excluded_windows.as_mut_ptr(),
            )
        }
        .as_bool(),
    )?;

    let requested_source = source_rect(source);
    bool_result(
        "MagSetWindowSource",
        unsafe { MagSetWindowSource(child, requested_source) }.as_bool(),
    )?;

    let mut actual_source = RECT::default();
    bool_result(
        "MagGetWindowSource",
        unsafe { MagGetWindowSource(child, &mut actual_source) }.as_bool(),
    )?;
    if rect_tuple(actual_source) != rect_tuple(requested_source) {
        return Err(MagnifierError {
            operation: "MagGetWindowSource readback",
            detail: format!(
                "requested={:?} actual={:?}",
                rect_tuple(requested_source),
                rect_tuple(actual_source)
            ),
        });
    }

    let mut actual_effect = MAGCOLOREFFECT::default();
    bool_result(
        "MagGetColorEffect",
        unsafe { MagGetColorEffect(child, &mut actual_effect) }.as_bool(),
    )?;
    if !effects_match(&effect, &actual_effect) {
        return Err(MagnifierError {
            operation: "MagGetColorEffect readback",
            detail: "driver returned a different color matrix".to_string(),
        });
    }

    unsafe { SetWindowPos(child, None, 0, 0, width, height, SHOW_FLAGS) }.map_err(|error| {
        MagnifierError {
            operation: "SetWindowPos(Magnifier show)",
            detail: error.to_string(),
        }
    })?;
    if !unsafe { IsWindowVisible(child) }.as_bool() {
        return Err(MagnifierError {
            operation: "IsWindowVisible(Magnifier)",
            detail: "configured child remained hidden".to_string(),
        });
    }

    let refresh_timer = unsafe {
        SetTimer(
            Some(child),
            SOURCE_REFRESH_TIMER_ID,
            SOURCE_REFRESH_INTERVAL_MS,
            Some(refresh_source_timer),
        )
    };
    if refresh_timer == 0 {
        return Err(MagnifierError {
            operation: "SetTimer(Magnifier source refresh)",
            detail: windows::core::Error::from_thread().to_string(),
        });
    }

    Ok(MagnifierReadback {
        host: host.0 as isize,
        source,
        gain: profile.gain,
        profile,
        child: child.0 as isize,
        excluded: excluded_raw,
        refresh_interval_ms: SOURCE_REFRESH_INTERVAL_MS,
    })
}

unsafe extern "system" fn refresh_source_timer(
    child: HWND,
    _message: u32,
    timer_id: usize,
    _time: u32,
) {
    if timer_id != SOURCE_REFRESH_TIMER_ID || child.0.is_null() {
        return;
    }
    let mut source = RECT::default();
    if unsafe { MagGetWindowSource(child, &mut source) }.as_bool() {
        let _ = unsafe { MagSetWindowSource(child, source) };
    }
}

pub(crate) fn destroy(host: isize) -> Result<(), MagnifierError> {
    if host == 0 {
        return Ok(());
    }
    if let Some(child) = find_child(hwnd(host)) {
        let _ = unsafe { KillTimer(Some(child), SOURCE_REFRESH_TIMER_ID) };
        unsafe { DestroyWindow(child) }.map_err(|error| MagnifierError {
            operation: "DestroyWindow(Magnifier)",
            detail: error.to_string(),
        })?;
    }
    Ok(())
}

pub(crate) fn is_configured(host: isize) -> bool {
    host != 0 && find_child(hwnd(host)).is_some()
}

fn ensure_initialized() -> Result<(), MagnifierError> {
    let result = INITIALIZED.get_or_init(|| {
        if unsafe { MagInitialize() }.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_thread().to_string())
        }
    });
    result.clone().map_err(|detail| MagnifierError {
        operation: "MagInitialize",
        detail,
    })
}

fn find_child(host: HWND) -> Option<HWND> {
    let child = unsafe { FindWindowExW(Some(host), None, WC_MAGNIFIER, CHILD_TITLE) }.ok()?;
    unsafe { IsWindow(Some(child)) }.as_bool().then_some(child)
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut c_void)
}

fn normalized_exclusions(host: isize, excluded: &[isize]) -> Vec<isize> {
    let mut normalized = Vec::with_capacity(excluded.len() + 1);
    normalized.push(host);
    normalized.extend(excluded.iter().copied().filter(|raw| *raw != 0));
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}

fn identity_transform() -> MAGTRANSFORM {
    MAGTRANSFORM {
        v: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    }
}

fn color_effect(profile: MagnifierProfile) -> MAGCOLOREFFECT {
    let luma = profile.cross_channel_luma;
    let red = 0.2126 * luma;
    let green = 0.7152 * luma;
    let blue = 0.0722 * luma;
    let translation = profile.black_translation;
    MAGCOLOREFFECT {
        transform: [
            profile.gain + red,
            red,
            red,
            0.0,
            0.0,
            green,
            profile.gain + green,
            green,
            0.0,
            0.0,
            blue,
            blue,
            profile.gain + blue,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            translation,
            translation,
            translation,
            0.0,
            1.0,
        ],
    }
}

fn source_rect(source: (i32, i32, i32, i32)) -> RECT {
    let (left, top, width, height) = source;
    RECT {
        left,
        top,
        right: left.saturating_add(width),
        bottom: top.saturating_add(height),
    }
}

fn rect_tuple(rect: RECT) -> (i32, i32, i32, i32) {
    (rect.left, rect.top, rect.right, rect.bottom)
}

fn effects_match(expected: &MAGCOLOREFFECT, actual: &MAGCOLOREFFECT) -> bool {
    expected
        .transform
        .iter()
        .zip(actual.transform.iter())
        .all(|(left, right)| (*left - *right).abs() <= 0.0001)
}

fn bool_result(operation: &'static str, succeeded: bool) -> Result<(), MagnifierError> {
    if succeeded {
        Ok(())
    } else {
        Err(MagnifierError {
            operation,
            detail: windows::core::Error::from_thread().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::night_vision::visibility::VisibilityPreset;

    #[test]
    fn identity_spatial_transform_never_scales_the_game() {
        assert_eq!(
            super::identity_transform().v,
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn color_effect_multiplies_rgb_without_adding_gray() {
        let profile = super::MagnifierProfile {
            gain: 3.8,
            black_translation: 0.0,
            cross_channel_luma: 0.0,
        };
        assert_eq!(
            super::color_effect(profile).transform,
            [
                3.8, 0.0, 0.0, 0.0, 0.0, 0.0, 3.8, 0.0, 0.0, 0.0, 0.0, 0.0, 3.8, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]
        );
    }

    #[test]
    fn fallback_profiles_are_bounded_monotonic_and_ordered() {
        let mut previous = [1.0_f32; 3];
        for strength in 0..=100 {
            let profiles = [
                super::fallback_profile(VisibilityPreset::Balanced, strength),
                super::fallback_profile(VisibilityPreset::Clear, strength),
                super::fallback_profile(VisibilityPreset::Ultra, strength),
            ];
            for (index, profile) in profiles.into_iter().enumerate() {
                assert!(profile.gain.is_finite() && (1.0..=6.0).contains(&profile.gain));
                assert!(
                    profile.black_translation.is_finite()
                        && (0.0..=0.08).contains(&profile.black_translation)
                );
                assert!(
                    profile.cross_channel_luma.is_finite()
                        && (0.0..=0.08).contains(&profile.cross_channel_luma)
                );
                assert!(profile.gain >= previous[index]);
                previous[index] = profile.gain;
            }
            assert!(profiles[0].gain <= profiles[1].gain);
            assert!(profiles[1].gain <= profiles[2].gain);
            assert!(profiles[0].black_translation <= profiles[1].black_translation);
            assert!(profiles[1].black_translation <= profiles[2].black_translation);
        }
        assert_eq!(
            super::fallback_profile(VisibilityPreset::Ultra, u8::MAX),
            super::fallback_profile(VisibilityPreset::Ultra, 100)
        );
    }

    #[test]
    fn fallback_color_matrix_places_luma_mix_and_translation_exactly() {
        let profile = super::MagnifierProfile {
            gain: 4.5,
            black_translation: 0.04,
            cross_channel_luma: 0.05,
        };
        let effect = super::color_effect(profile);
        let c = profile.cross_channel_luma;
        assert_eq!(
            effect.transform,
            [
                profile.gain + 0.2126 * c,
                0.2126 * c,
                0.2126 * c,
                0.0,
                0.0,
                0.7152 * c,
                profile.gain + 0.7152 * c,
                0.7152 * c,
                0.0,
                0.0,
                0.0722 * c,
                0.0722 * c,
                profile.gain + 0.0722 * c,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                profile.black_translation,
                profile.black_translation,
                profile.black_translation,
                0.0,
                1.0,
            ]
        );
    }

    #[test]
    fn source_rectangle_uses_desktop_coordinates_and_size() {
        let rect = super::source_rect((10, 20, 300, 400));
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (10, 20, 310, 420)
        );
    }

    #[test]
    fn native_source_refresh_tracks_a_sixty_fps_frame_cadence() {
        assert!((15..=17).contains(&super::SOURCE_REFRESH_INTERVAL_MS));
    }

    #[test]
    fn exclusion_fingerprint_is_sorted_deduplicated_and_includes_host() {
        assert_eq!(
            super::normalized_exclusions(30, &[50, 0, 30, 10, 50]),
            vec![10, 30, 50]
        );
    }
}
