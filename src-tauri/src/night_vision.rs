mod curve;
mod gpu;
mod magnifier;
mod recovery;
pub mod visibility;
mod windows;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Listener, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::settings::GAME_PROCESS_NAME;
use crate::state::AppState;
use crate::state::LockExt;

pub(crate) use curve::GammaRamp;
use visibility::{RendererReadback, VisibilityPreset, VisibilityRenderer};
pub(crate) use windows::NightVisionError;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GameTarget {
    pub(crate) hwnd: isize,
    pub(crate) display_name: String,
}

pub const CHANGED_EVENT: &str = "night-vision://changed";
pub const BUILD_FINGERPRINT: &str = concat!(env!("CARGO_PKG_VERSION"), "-gpu-visibility-c");
const FILTER_LABEL: &str = "night-vision-filter";
const FILTER_READY_EVENT: &str = "night-vision-filter://ready";
const FILTER_HEARTBEAT_EVENT: &str = "night-vision-filter://heartbeat";
const OVERLAY_STACK_LABELS: [&str; 5] = [
    FILTER_LABEL,
    "minimap",
    "hud",
    "water-guide",
    "night-vision",
];
const CAPTURE_EXCLUDED_LABELS: [&str; 6] = [
    FILTER_LABEL,
    "main",
    "minimap",
    "hud",
    "water-guide",
    "night-vision",
];
const BUTTON_WIDTH: f64 = 190.0;
const BUTTON_HEIGHT: f64 = 48.0;
const BUTTON_MARGIN: f64 = 16.0;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NightVisionState {
    pub requested: bool,
    pub applied: bool,
    pub supported: bool,
    pub strength: u8,
    pub error_key: Option<String>,
    pub visual_boost_ready: bool,
    pub visual_boost_applied: bool,
    pub gamma_applied: bool,
    pub renderer: VisibilityRenderer,
    pub preset: VisibilityPreset,
    pub force_bright: bool,
    pub prefer_gpu: bool,
    pub scene_luma: Option<f32>,
    pub presented_fps: Option<f32>,
    pub build_fingerprint: &'static str,
}

#[cfg(test)]
pub(crate) fn visual_boost_gain(strength: u8) -> f32 {
    1.0 + 4.0 * f32::from(strength.min(100)) / 100.0
}

struct ControllerInner {
    state: NightVisionState,
    recovery_blocked: bool,
    next_visual_request: u64,
    pending_visual_request: Option<(u64, u8)>,
}

pub(crate) struct NightVisionController {
    inner: Mutex<ControllerInner>,
    native_generation: Arc<AtomicU64>,
}

impl NightVisionController {
    pub(crate) fn new(strength: u8) -> Self {
        Self {
            native_generation: Arc::new(AtomicU64::new(0)),
            inner: Mutex::new(ControllerInner {
                state: NightVisionState {
                    requested: false,
                    applied: false,
                    supported: true,
                    strength: strength.min(100),
                    error_key: None,
                    visual_boost_ready: false,
                    visual_boost_applied: false,
                    gamma_applied: false,
                    renderer: VisibilityRenderer::None,
                    preset: VisibilityPreset::Ultra,
                    force_bright: true,
                    prefer_gpu: true,
                    scene_luma: None,
                    presented_fps: None,
                    build_fingerprint: BUILD_FINGERPRINT,
                },
                recovery_blocked: false,
                next_visual_request: 0,
                pending_visual_request: None,
            }),
        }
    }

    pub(crate) fn state(&self) -> NightVisionState {
        self.inner.lock_safe().state.clone()
    }

    fn invalidate_native_generation(&self) {
        self.native_generation.store(0, Ordering::SeqCst);
    }

    fn native_generation(&self) -> Arc<AtomicU64> {
        self.native_generation.clone()
    }

    pub(crate) fn toggle_requested(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        if inner.recovery_blocked {
            return inner.state.clone();
        }
        inner.state.requested = !inner.state.requested;
        inner.pending_visual_request = None;
        self.invalidate_native_generation();
        if !inner.state.visual_boost_applied {
            inner.state.applied = false;
            inner.state.error_key = None;
            if inner.state.requested {
                inner.state.supported = true;
            }
        }
        inner.state.clone()
    }

    pub(crate) fn block_for_recovery_error(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.recovery_blocked = true;
        inner.pending_visual_request = None;
        self.invalidate_native_generation();
        inner.state.requested = false;
        inner.state.applied = false;
        inner.state.visual_boost_applied = false;
        inner.state.gamma_applied = false;
        inner.state.renderer = VisibilityRenderer::None;
        inner.state.scene_luma = None;
        inner.state.presented_fps = None;
        inner.state.supported = false;
        inner.state.error_key = Some("night_vision.recovery_error".to_string());
        inner.state.clone()
    }

    pub(crate) fn set_strength(&self, strength: u8) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        let strength = strength.min(100);
        if inner.state.strength != strength {
            inner.state.strength = strength;
            inner.pending_visual_request = None;
            self.invalidate_native_generation();
        }
        inner.state.clone()
    }

    pub(crate) fn set_preset(&self, preset: VisibilityPreset) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        if inner.state.preset != preset {
            inner.state.preset = preset;
            inner.pending_visual_request = None;
            self.invalidate_native_generation();
        }
        inner.state.clone()
    }

    pub(crate) fn set_force_bright(&self, force_bright: bool) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        if inner.state.force_bright != force_bright {
            inner.state.force_bright = force_bright;
            inner.pending_visual_request = None;
            self.invalidate_native_generation();
        }
        inner.state.clone()
    }

    pub(crate) fn set_prefer_gpu(&self, prefer_gpu: bool) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        if inner.state.prefer_gpu != prefer_gpu {
            inner.state.prefer_gpu = prefer_gpu;
            inner.pending_visual_request = None;
            self.invalidate_native_generation();
        }
        inner.state.clone()
    }

    pub(crate) fn mark_filter_ready(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.state.visual_boost_ready = true;
        if !inner.recovery_blocked {
            inner.state.supported = true;
            if inner.state.error_key.as_deref() == Some("night_vision.filter_unavailable") {
                inner.state.error_key = None;
            }
        }
        inner.state.clone()
    }

    pub(crate) fn begin_visual_request(&self) -> Option<(u64, u8)> {
        let mut inner = self.inner.lock_safe();
        if inner.recovery_blocked || !inner.state.requested || !inner.state.visual_boost_ready {
            return None;
        }
        if let Some((request_id, strength)) = inner.pending_visual_request {
            if strength == inner.state.strength {
                return Some((request_id, strength));
            }
        }
        inner.next_visual_request = inner.next_visual_request.wrapping_add(1).max(1);
        let request_id = inner.next_visual_request;
        let strength = inner.state.strength;
        inner.pending_visual_request = Some((request_id, strength));
        self.native_generation.store(request_id, Ordering::SeqCst);
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        Some((request_id, strength))
    }

    pub(crate) fn accept_native_visual(
        &self,
        request_id: u64,
        strength: u8,
        native_verified: bool,
        window_visible: bool,
    ) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        let expected = inner.pending_visual_request;
        if expected == Some((request_id, strength.min(100)))
            && inner.state.requested
            && inner.state.visual_boost_ready
            && native_verified
            && window_visible
        {
            inner.pending_visual_request = None;
            inner.state.visual_boost_applied = true;
            inner.state.applied = true;
            inner.state.renderer = VisibilityRenderer::MagnifierFallback;
            inner.state.scene_luma = None;
            inner.state.presented_fps = None;
            inner.state.supported = true;
            if matches!(
                inner.state.error_key.as_deref(),
                Some("night_vision.waiting_for_game" | "night_vision.filter_unavailable")
            ) {
                inner.state.error_key = None;
            }
        }
        inner.state.clone()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn accept_renderer_readback(
        &self,
        request_id: u64,
        strength: u8,
        expected_game_hwnd: isize,
        expected_source: (i32, i32, i32, i32),
        readback: RendererReadback,
        now: Instant,
        window_visible: bool,
    ) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        let expected = inner.pending_visual_request;
        let verified = readback.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            expected_game_hwnd,
            expected_source,
            inner.state.preset,
            now,
        );
        if expected == Some((request_id, strength.min(100)))
            && inner.state.requested
            && inner.state.visual_boost_ready
            && verified
            && window_visible
        {
            inner.pending_visual_request = None;
            inner.state.visual_boost_applied = true;
            inner.state.applied = true;
            inner.state.supported = true;
            inner.state.renderer = VisibilityRenderer::GpuAdaptive;
            inner.state.scene_luma = Some(readback.scene_luma);
            inner.state.presented_fps = Some(1000.0 / readback.median_interval_ms);
            if matches!(
                inner.state.error_key.as_deref(),
                Some("night_vision.waiting_for_game" | "night_vision.filter_unavailable")
            ) {
                inner.state.error_key = None;
            }
        }
        inner.state.clone()
    }

    pub(crate) fn refresh_renderer_readback(
        &self,
        expected_game_hwnd: isize,
        expected_source: (i32, i32, i32, i32),
        readback: RendererReadback,
        now: Instant,
        window_visible: bool,
    ) -> Option<NightVisionState> {
        let mut inner = self.inner.lock_safe();
        let verified = readback.is_current_for(
            VisibilityRenderer::GpuAdaptive,
            expected_game_hwnd,
            expected_source,
            inner.state.preset,
            now,
        );
        if !inner.state.requested
            || !inner.state.visual_boost_applied
            || inner.state.renderer != VisibilityRenderer::GpuAdaptive
            || !verified
            || !window_visible
        {
            return None;
        }
        inner.state.scene_luma = Some(readback.scene_luma);
        inner.state.presented_fps = Some(1000.0 / readback.median_interval_ms);
        Some(inner.state.clone())
    }

    pub(crate) fn confirm_visual_hidden(&self, error_key: Option<&str>) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.pending_visual_request = None;
        self.invalidate_native_generation();
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        inner.state.renderer = VisibilityRenderer::None;
        inner.state.scene_luma = None;
        inner.state.presented_fps = None;
        inner.state.error_key = if inner.state.requested {
            error_key.map(str::to_string)
        } else {
            None
        };
        inner.state.clone()
    }

    pub(crate) fn mark_filter_failed(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.pending_visual_request = None;
        self.invalidate_native_generation();
        inner.state.visual_boost_ready = false;
        inner.state.renderer = VisibilityRenderer::None;
        inner.state.scene_luma = None;
        inner.state.presented_fps = None;
        inner.state.supported = false;
        inner.state.error_key = Some("night_vision.filter_unavailable".to_string());
        inner.state.clone()
    }

    pub(crate) fn mark_filter_cleanup_failed(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.pending_visual_request = None;
        self.invalidate_native_generation();
        inner.state.visual_boost_applied = true;
        inner.state.applied = true;
        inner.state.supported = false;
        inner.state.error_key = Some("night_vision.filter_cleanup_error".to_string());
        inner.state.clone()
    }

    pub(crate) fn reconcile(&self, game: Option<GameTarget>) -> NightVisionState {
        let mut inner = self.inner.lock_safe();

        if inner.recovery_blocked {
            return inner.state.clone();
        }

        if !inner.state.requested {
            inner.state.gamma_applied = false;
            return inner.state.clone();
        }

        if game.is_none() {
            if !inner.state.visual_boost_applied {
                inner.state.error_key = Some("night_vision.waiting_for_game".to_string());
            }
            return inner.state.clone();
        }

        inner.state.gamma_applied = false;
        if inner.state.visual_boost_ready {
            inner.state.supported = true;
            if !inner.state.visual_boost_applied {
                inner.state.error_key = None;
            }
        } else if !inner.state.visual_boost_applied {
            inner.state.supported = false;
            inner.state.error_key = Some("night_vision.filter_unavailable".to_string());
        }

        inner.state.clone()
    }

    pub(crate) fn finish_exit(&self, cleanup_verified: bool) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.state.requested = false;
        inner.pending_visual_request = None;
        self.invalidate_native_generation();
        inner.state.gamma_applied = false;
        if cleanup_verified {
            inner.state.visual_boost_applied = false;
            inner.state.applied = false;
            inner.state.renderer = VisibilityRenderer::None;
            inner.state.scene_luma = None;
            inner.state.presented_fps = None;
            inner.state.error_key = None;
        } else {
            inner.state.visual_boost_applied = true;
            inner.state.applied = true;
            inner.state.supported = false;
            inner.state.error_key = Some("night_vision.filter_cleanup_error".to_string());
        }
        inner.state.clone()
    }
}

pub struct NightVision {
    controller: NightVisionController,
    gpu_session: Mutex<Option<gpu::GpuVisibilitySession>>,
}

impl Default for NightVision {
    fn default() -> Self {
        Self::new()
    }
}

impl NightVision {
    pub fn new() -> Self {
        Self {
            controller: NightVisionController::new(85),
            gpu_session: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn get_night_vision_state(night_vision: State<'_, NightVision>) -> NightVisionState {
    night_vision.controller.state()
}

#[tauri::command]
pub fn toggle_night_vision(
    app: AppHandle,
    night_vision: State<'_, NightVision>,
) -> NightVisionState {
    night_vision.controller.toggle_requested();
    let state = night_vision.controller.reconcile(active_game_target());
    emit_state(&app, &state);
    state
}

#[tauri::command]
pub fn set_night_vision_strength(
    app: AppHandle,
    night_vision: State<'_, NightVision>,
    strength: u8,
) -> NightVisionState {
    let strength = strength.min(100);
    crate::commands::apply_settings_patch(
        &app,
        serde_json::json!({ "night_vision": { "strength": strength } }),
    );
    night_vision.controller.set_strength(strength);
    let state = night_vision.controller.reconcile(active_game_target());
    emit_state(&app, &state);
    state
}

#[tauri::command]
pub fn set_night_vision_preset(
    app: AppHandle,
    night_vision: State<'_, NightVision>,
    preset: VisibilityPreset,
) -> NightVisionState {
    crate::commands::apply_settings_patch(
        &app,
        serde_json::json!({ "night_vision": { "preset": preset } }),
    );
    night_vision.controller.set_preset(preset);
    let state = night_vision.controller.reconcile(active_game_target());
    emit_state(&app, &state);
    state
}

#[tauri::command]
pub fn set_night_vision_force_bright(
    app: AppHandle,
    night_vision: State<'_, NightVision>,
    force_bright: bool,
) -> NightVisionState {
    crate::commands::apply_settings_patch(
        &app,
        serde_json::json!({ "night_vision": { "force_bright": force_bright } }),
    );
    night_vision.controller.set_force_bright(force_bright);
    let state = night_vision.controller.reconcile(active_game_target());
    emit_state(&app, &state);
    state
}

pub fn toggle_from_app(app: &AppHandle) {
    let night_vision = app.state::<NightVision>();
    night_vision.controller.toggle_requested();
    let state = night_vision.controller.reconcile(active_game_target());
    emit_state(app, &state);
}

pub fn initialize(app: &AppHandle) {
    let night_vision = app.state::<NightVision>();
    let (strength, preset, force_bright, prefer_gpu) = {
        let app_state = app.state::<AppState>();
        let settings = app_state.settings.lock_safe();
        let strength = crate::settings::get_f64(&settings, &["night_vision", "strength"], 85.0)
            .round()
            .clamp(0.0, 100.0) as u8;
        let preset = match crate::settings::get_str(&settings, &["night_vision", "preset"], "ultra")
        {
            "balanced" => VisibilityPreset::Balanced,
            "clear" => VisibilityPreset::Clear,
            _ => VisibilityPreset::Ultra,
        };
        let force_bright =
            crate::settings::get_bool(&settings, &["night_vision", "force_bright"], true);
        let prefer_gpu =
            crate::settings::get_bool(&settings, &["night_vision", "prefer_gpu"], true);
        (strength, preset, force_bright, prefer_gpu)
    };
    night_vision.controller.set_strength(strength);
    night_vision.controller.set_preset(preset);
    night_vision.controller.set_force_bright(force_bright);
    night_vision.controller.set_prefer_gpu(prefer_gpu);

    match windows::restore_recovery_record(&crate::settings::night_vision_recovery_path()) {
        Ok(true) => log::info!("night vision: restored gamma from crash recovery record"),
        Ok(false) => {}
        Err(NightVisionError::RecoveryCleanup(error)) => {
            log::warn!("night vision: gamma restored but recovery cleanup is pending: {error}");
        }
        Err(error) => {
            log::error!("night vision crash recovery failed; feature blocked: {error}");
            let state = night_vision.controller.block_for_recovery_error();
            emit_state(app, &state);
        }
    }

    spawn_supervisor(app.clone());
}

pub fn restore_before_exit(app: &AppHandle) -> NightVisionState {
    let night_vision = app.state::<NightVision>();
    let filter_hidden = hide_filter_window(app);
    let state = night_vision.controller.finish_exit(filter_hidden);
    if !filter_hidden {
        log::error!("night vision: visual filter hide on exit was not verified");
    }
    emit_state(app, &state);
    state
}

#[tauri::command]
pub fn prepare_night_vision_exit(app: AppHandle) -> NightVisionState {
    restore_before_exit(&app)
}

static UI_THREAD: OnceLock<std::thread::ThreadId> = OnceLock::new();

fn on_ui_thread<T, F>(app: &AppHandle, operation: &'static str, call: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    if UI_THREAD.get() == Some(&std::thread::current().id()) {
        return call();
    }

    let (sender, receiver) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = sender.send(call());
    })
    .map_err(|error| format!("schedule {operation}: {error}"))?;
    receiver
        .recv_timeout(Duration::from_millis(1_000))
        .map_err(|error| format!("wait for {operation}: {error}"))?
}

fn configure_magnifier(
    app: &AppHandle,
    request_id: u64,
    source: (i32, i32, i32, i32),
    strength: u8,
    excluded: Vec<isize>,
) -> Result<magnifier::MagnifierReadback, String> {
    let host = crate::win::vis::hwnd(FILTER_LABEL)
        .ok_or_else(|| "night vision host HWND is not registered".to_string())?;
    let preset = app.state::<NightVision>().controller.state().preset;
    let profile = magnifier::fallback_profile(preset, strength);
    let generation = app.state::<NightVision>().controller.native_generation();
    let ui_generation = generation.clone();
    let result = on_ui_thread(app, "native magnifier configure", move || {
        if ui_generation.load(Ordering::SeqCst) != request_id {
            return Err("native magnifier request was superseded before apply".to_string());
        }
        let result = magnifier::configure(host, source, profile, &excluded)
            .map_err(|error| error.to_string());
        if ui_generation.load(Ordering::SeqCst) != request_id {
            let _ = magnifier::destroy(host);
            return Err("native magnifier request was superseded after apply".to_string());
        }
        result
    });
    if result.is_err() {
        let _ = generation.compare_exchange(request_id, 0, Ordering::SeqCst, Ordering::SeqCst);
        let _ = destroy_magnifier(app);
    }
    result
}

fn configure_gpu(
    app: &AppHandle,
    request_id: u64,
    game_hwnd: isize,
    source: (i32, i32, i32, i32),
    strength: u8,
) -> Result<RendererReadback, String> {
    let host = crate::win::vis::hwnd(FILTER_LABEL)
        .ok_or_else(|| "night vision host HWND is not registered".to_string())?;
    let night_vision = app.state::<NightVision>();
    let state = night_vision.controller.state();
    let output = on_ui_thread(app, "GPU output window create", move || {
        gpu::create_output_window(host, source).map_err(|error| error.to_string())
    })?;
    let config = match gpu::GpuSessionConfig::new(
        host,
        output,
        game_hwnd,
        source,
        state.preset,
        strength,
        state.force_bright,
    ) {
        Ok(config) => config,
        Err(error) => {
            let _ = destroy_gpu_output(app, output);
            return Err(error);
        }
    };
    let generation = night_vision.controller.native_generation();
    if generation.load(Ordering::SeqCst) != request_id {
        let _ = destroy_gpu_output(app, output);
        return Err("GPU visibility request was superseded before startup".to_string());
    }

    let session = match gpu::GpuVisibilitySession::start(config) {
        Ok(session) => session,
        Err(error) => {
            let _ = destroy_gpu_output(app, output);
            return Err(error.to_string());
        }
    };
    let deadline = Instant::now() + Duration::from_millis(2_500);
    loop {
        if generation.load(Ordering::SeqCst) != request_id {
            let _ = session.stop();
            let _ = destroy_gpu_output(app, output);
            return Err("GPU visibility request was superseded during startup".to_string());
        }
        match session.readback() {
            Err(error) => {
                let _ = session.stop();
                let _ = destroy_gpu_output(app, output);
                return Err(error.to_string());
            }
            Ok(Some(readback)) => {
                *night_vision.gpu_session.lock_safe() = Some(session);
                return Ok(readback);
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = session.stop();
                let _ = destroy_gpu_output(app, output);
                return Err("GPU visibility did not present a frame within 2500ms".to_string());
            }
        }
    }
}

fn configure_and_accept_magnifier(
    app: &AppHandle,
    request_id: u64,
    source: (i32, i32, i32, i32),
    strength: u8,
    excluded: Vec<isize>,
) -> Result<(NightVisionState, magnifier::MagnifierReadback), String> {
    let readback = configure_magnifier(app, request_id, source, strength, excluded)?;
    let visible = crate::win::vis::hwnd(FILTER_LABEL) == Some(readback.host)
        && crate::win::vis::is_visible(FILTER_LABEL) == Some(true)
        && current_excluded_windows() == readback.excluded;
    let state = app
        .state::<NightVision>()
        .controller
        .accept_native_visual(request_id, strength, true, visible);
    if state.visual_boost_applied {
        Ok((state, readback))
    } else {
        let _ = destroy_magnifier(app);
        Err("native magnifier readback did not match the active request".to_string())
    }
}

fn gpu_readback(app: &AppHandle) -> Result<Option<RendererReadback>, String> {
    let night_vision = app.state::<NightVision>();
    let sessions = night_vision.gpu_session.lock_safe();
    let Some(session) = sessions.as_ref() else {
        return Ok(None);
    };
    session.readback().map_err(|error| error.to_string())
}

fn has_gpu_session(app: &AppHandle) -> bool {
    app.state::<NightVision>().gpu_session.lock_safe().is_some()
}

fn destroy_gpu_output(app: &AppHandle, output_hwnd: isize) -> bool {
    match on_ui_thread(app, "GPU output window destroy", move || {
        gpu::destroy_output_window(output_hwnd).map_err(|error| error.to_string())
    }) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("night vision: GPU output cleanup failed: {error}");
            false
        }
    }
}

fn destroy_gpu(app: &AppHandle) -> bool {
    let session = app.state::<NightVision>().gpu_session.lock_safe().take();
    let Some(session) = session else {
        return true;
    };
    let output = session.output_hwnd();
    match session.stop() {
        Ok(()) => destroy_gpu_output(app, output),
        Err(error) => {
            log::warn!("night vision: GPU cleanup failed: {error}");
            let _ = destroy_gpu_output(app, output);
            false
        }
    }
}

fn current_excluded_windows() -> Vec<isize> {
    let mut excluded: Vec<isize> = CAPTURE_EXCLUDED_LABELS
        .into_iter()
        .filter_map(crate::win::vis::hwnd)
        .filter(|raw| *raw != 0)
        .collect();
    excluded.sort_unstable();
    excluded.dedup();
    excluded
}

fn destroy_magnifier(app: &AppHandle) -> bool {
    app.state::<NightVision>()
        .controller
        .invalidate_native_generation();
    let Some(host) = crate::win::vis::hwnd(FILTER_LABEL) else {
        return true;
    };
    if !magnifier::is_configured(host) {
        return true;
    }
    match on_ui_thread(app, "native magnifier destroy", move || {
        magnifier::destroy(host).map_err(|error| error.to_string())
    }) {
        Ok(()) => !magnifier::is_configured(host),
        Err(error) => {
            log::warn!("night vision: native magnifier cleanup failed: {error}");
            false
        }
    }
}

pub fn create_filter(app: &AppHandle) -> tauri::Result<()> {
    let _ = UI_THREAD.set(std::thread::current().id());
    let health = Arc::new(WindowHealth::new());

    let ready_health = health.clone();
    let ready_app = app.clone();
    app.listen_any(FILTER_READY_EVENT, move |_| {
        ready_health.mark_ready();
        let state = ready_app
            .state::<NightVision>()
            .controller
            .mark_filter_ready();
        emit_state(&ready_app, &state);
    });

    let heartbeat_health = health.clone();
    app.listen_any(FILTER_HEARTBEAT_EVENT, move |_| {
        heartbeat_health.mark_heartbeat();
    });

    build_filter_window(app, &health)?;
    spawn_filter_supervisor(app.clone(), health);
    Ok(())
}

fn build_filter_window(
    app: &AppHandle,
    health: &WindowHealth,
) -> tauri::Result<tauri::WebviewWindow> {
    health.reset();
    let window = WebviewWindowBuilder::new(
        app,
        FILTER_LABEL,
        WebviewUrl::App("night-vision-filter.html".into()),
    )
    .title("night vision filter")
    .inner_size(1.0, 1.0)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .focusable(false)
    .visible(false)
    .build()?;

    let hwnd = window.hwnd()?;
    let raw = hwnd.0 as isize;
    crate::win::vis::register(FILTER_LABEL, raw);
    crate::win::overlay::assert_overlay_styles(raw);
    crate::win::overlay::set_click_through(raw, true);
    window.set_ignore_cursor_events(true)?;
    Ok(window)
}

fn filter_should_show(
    requested: bool,
    ready: bool,
    game_active: bool,
    main_in_front: bool,
) -> bool {
    requested && ready && game_active && !main_in_front
}

fn wait_filter_hidden(timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if crate::win::vis::is_visible(FILTER_LABEL) != Some(true) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn hide_filter_window(app: &AppHandle) -> bool {
    let Some(window) = app.get_webview_window(FILTER_LABEL) else {
        return destroy_gpu(app)
            && destroy_magnifier(app)
            && crate::win::vis::is_visible(FILTER_LABEL) != Some(true);
    };
    let gpu_destroyed = destroy_gpu(app);
    let magnifier_destroyed = destroy_magnifier(app);
    crate::webview_mem::on_hidden(&window);
    let hide_requested = window.hide().is_ok();
    let host_hidden = wait_filter_hidden(250);
    gpu_destroyed && magnifier_destroyed && hide_requested && host_hidden
}

fn force_overlay_stack() {
    // SetWindowPos(HWND_TOPMOST) lifts each window above the previous one, so
    // keep the visual filter first and player-facing guidance last.
    for label in OVERLAY_STACK_LABELS {
        if let Some(hwnd) = crate::win::vis::hwnd(label) {
            crate::win::overlay::force_topmost(hwnd);
        }
    }
}

fn spawn_filter_supervisor(app: AppHandle, health: Arc<WindowHealth>) {
    std::thread::spawn(move || {
        const TICK_MS: u64 = 250;
        const GAME_SEARCH_MS: u64 = 1000;
        const RECREATE_MS: u64 = 5000;
        const APPLY_RETRY_MS: u64 = 1000;
        const TOPMOST_MS: u64 = 2000;

        let mut game_hwnd = None;
        let mut since_search = GAME_SEARCH_MS;
        let mut since_recreate = RECREATE_MS;
        let mut since_apply = APPLY_RETRY_MS;
        let mut since_topmost = TOPMOST_MS;
        let mut unfocused_ticks: u8 = 2;
        let mut effective_previous = false;
        let mut last_rect = None;
        let mut last_strength = None;
        let mut last_mode: Option<(VisibilityPreset, bool, bool)> = None;
        let mut last_excluded: Option<Vec<isize>> = None;

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            since_search = since_search.saturating_add(TICK_MS);
            since_recreate = since_recreate.saturating_add(TICK_MS);
            since_apply = since_apply.saturating_add(TICK_MS);
            since_topmost = since_topmost.saturating_add(TICK_MS);

            let Some(window) = app.get_webview_window(FILTER_LABEL) else {
                let state = if destroy_gpu(&app) && destroy_magnifier(&app) {
                    app.state::<NightVision>()
                        .controller
                        .confirm_visual_hidden(Some("night_vision.filter_unavailable"));
                    app.state::<NightVision>().controller.mark_filter_failed()
                } else {
                    app.state::<NightVision>()
                        .controller
                        .mark_filter_cleanup_failed()
                };
                emit_state(&app, &state);
                if since_recreate >= RECREATE_MS {
                    since_recreate = 0;
                    match build_filter_window(&app, &health) {
                        Ok(_) => {
                            effective_previous = false;
                            last_rect = None;
                            last_strength = None;
                            last_excluded = None;
                        }
                        Err(error) => {
                            log::warn!("night vision filter recreate failed: {error}");
                        }
                    }
                }
                continue;
            };
            since_recreate = 0;

            if since_search >= GAME_SEARCH_MS {
                since_search = 0;
                game_hwnd = crate::win::game_window::find_game_window(GAME_PROCESS_NAME);
            }
            let game_present =
                game_hwnd.is_some_and(|hwnd| !crate::win::game_window::is_iconic(hwnd));
            if game_present && game_hwnd.is_some_and(crate::win::game_window::is_foreground) {
                unfocused_ticks = 0;
            } else {
                unfocused_ticks = unfocused_ticks.saturating_add(1);
            }
            let game_active = game_present && unfocused_ticks < 2;
            let state = app.state::<NightVision>().controller.state();
            let (ready, heartbeat_age_ms) = health.snapshot();
            let effective = filter_should_show(
                state.requested,
                ready,
                game_active,
                crate::win::vis::is_foreground("main"),
            );

            if ready && heartbeat_age_ms >= 6_000 {
                log::warn!("night vision filter heartbeat stale; recreating owned window");
                if hide_filter_window(&app) {
                    let _ = window.close();
                    app.state::<NightVision>()
                        .controller
                        .confirm_visual_hidden(Some("night_vision.filter_unavailable"));
                    let failed = app.state::<NightVision>().controller.mark_filter_failed();
                    emit_state(&app, &failed);
                } else {
                    let failed = app
                        .state::<NightVision>()
                        .controller
                        .mark_filter_cleanup_failed();
                    emit_state(&app, &failed);
                }
                continue;
            }

            if !effective {
                let cleanup_needed = crate::win::vis::is_visible(FILTER_LABEL) == Some(true)
                    || state.visual_boost_applied
                    || has_gpu_session(&app)
                    || crate::win::vis::hwnd(FILTER_LABEL).is_some_and(magnifier::is_configured);
                if cleanup_needed {
                    let hidden = if hide_filter_window(&app) {
                        app.state::<NightVision>()
                            .controller
                            .confirm_visual_hidden(Some("night_vision.waiting_for_game"))
                    } else {
                        app.state::<NightVision>()
                            .controller
                            .mark_filter_cleanup_failed()
                    };
                    emit_state(&app, &hidden);
                } else if effective_previous {
                    let hidden = app
                        .state::<NightVision>()
                        .controller
                        .confirm_visual_hidden(Some("night_vision.waiting_for_game"));
                    emit_state(&app, &hidden);
                }
                effective_previous = false;
                last_rect = None;
                last_strength = None;
                last_excluded = None;
                continue;
            }

            let Some(rect) = game_hwnd
                .and_then(crate::win::game_window::client_rect_on_screen)
                .filter(|(_, _, width, height)| *width > 0 && *height > 0)
            else {
                continue;
            };

            if last_rect != Some(rect) {
                let (left, top, width, height) = rect;
                if window
                    .set_position(PhysicalPosition::new(left, top))
                    .and_then(|_| window.set_size(PhysicalSize::new(width as u32, height as u32)))
                    .is_err()
                {
                    let failed = app.state::<NightVision>().controller.mark_filter_failed();
                    emit_state(&app, &failed);
                    continue;
                }
                last_rect = Some(rect);
                last_strength = None;
                last_excluded = None;
                since_apply = APPLY_RETRY_MS;
            }

            if crate::win::vis::is_visible(FILTER_LABEL) != Some(true) {
                crate::webview_mem::on_shown(&window);
                if window.show().is_err() || !crate::win::vis::wait_visible(FILTER_LABEL, 500) {
                    let failed = app.state::<NightVision>().controller.mark_filter_failed();
                    emit_state(&app, &failed);
                    continue;
                }
                force_overlay_stack();
                since_apply = APPLY_RETRY_MS;
            }

            let mut current = app.state::<NightVision>().controller.state();
            if current.visual_boost_applied && current.renderer == VisibilityRenderer::GpuAdaptive {
                let visible = crate::win::vis::is_visible(FILTER_LABEL) == Some(true);
                let refreshed = gpu_readback(&app).ok().flatten().and_then(|readback| {
                    app.state::<NightVision>()
                        .controller
                        .refresh_renderer_readback(
                            game_hwnd.unwrap_or_default(),
                            rect,
                            readback,
                            Instant::now(),
                            visible,
                        )
                });
                if let Some(state) = refreshed {
                    if state.scene_luma != current.scene_luma
                        || state.presented_fps != current.presented_fps
                    {
                        emit_state(&app, &state);
                    }
                    current = state;
                } else {
                    log::warn!(
                        "night vision: GPU readback became stale or mismatched; restarting renderer"
                    );
                    current = if destroy_gpu(&app) {
                        app.state::<NightVision>()
                            .controller
                            .confirm_visual_hidden(None)
                    } else {
                        app.state::<NightVision>()
                            .controller
                            .mark_filter_cleanup_failed()
                    };
                    emit_state(&app, &current);
                    last_strength = None;
                    last_excluded = None;
                    since_apply = APPLY_RETRY_MS;
                }
            }
            let excluded = current_excluded_windows();
            if (!current.visual_boost_applied
                || last_strength != Some(current.strength)
                || last_mode != Some((current.preset, current.force_bright, current.prefer_gpu))
                || last_excluded.as_ref() != Some(&excluded))
                && since_apply >= APPLY_RETRY_MS
            {
                since_apply = 0;
                if !destroy_gpu(&app) || !destroy_magnifier(&app) {
                    let failed = app
                        .state::<NightVision>()
                        .controller
                        .mark_filter_cleanup_failed();
                    emit_state(&app, &failed);
                    last_strength = None;
                    last_excluded = None;
                    continue;
                }
                last_strength = None;
                last_excluded = None;
                let pending = app
                    .state::<NightVision>()
                    .controller
                    .confirm_visual_hidden(None);
                emit_state(&app, &pending);
                if let Some((request_id, request_strength)) =
                    app.state::<NightVision>().controller.begin_visual_request()
                {
                    let game = game_hwnd.unwrap_or_default();
                    let gpu_result = if current.prefer_gpu {
                        configure_gpu(&app, request_id, game, rect, request_strength)
                    } else {
                        Err("GPU renderer disabled by user setting".to_string())
                    };
                    match gpu_result {
                        Ok(readback) => {
                            let visible = crate::win::vis::is_visible(FILTER_LABEL) == Some(true);
                            let state = app
                                .state::<NightVision>()
                                .controller
                                .accept_renderer_readback(
                                    request_id,
                                    request_strength,
                                    game,
                                    rect,
                                    readback,
                                    Instant::now(),
                                    visible,
                                );
                            if state.visual_boost_applied {
                                log::info!(
                                    "night vision: adaptive GPU renderer verified request={} strength={} preset={:?} frames={} luma={:.4} fps={:.1} source={:?} fingerprint={}",
                                    request_id,
                                    request_strength,
                                    state.preset,
                                    readback.presented_frames,
                                    readback.scene_luma,
                                    1000.0 / readback.median_interval_ms,
                                    readback.source,
                                    state.build_fingerprint
                                );
                                last_strength = Some(request_strength);
                                last_mode =
                                    Some((state.preset, state.force_bright, state.prefer_gpu));
                                last_excluded = Some(excluded.clone());
                                emit_state(&app, &state);
                            } else if destroy_gpu(&app) {
                                let hidden = app
                                    .state::<NightVision>()
                                    .controller
                                    .confirm_visual_hidden(None);
                                emit_state(&app, &hidden);
                            } else {
                                let failed = app
                                    .state::<NightVision>()
                                    .controller
                                    .mark_filter_cleanup_failed();
                                emit_state(&app, &failed);
                            }
                        }
                        Err(error) => {
                            log::warn!(
                                "night vision: adaptive GPU renderer unavailable; using truthful magnifier fallback: {error}"
                            );
                            match configure_and_accept_magnifier(
                                &app,
                                request_id,
                                rect,
                                request_strength,
                                excluded,
                            ) {
                                Ok((state, readback)) => {
                                    log::info!(
                                        "night vision: native magnifier fallback verified request={} strength={} preset={:?} gain={:.2} black_translation={:.3} luma_mix={:.3} child={} source={:?} refresh={}ms fingerprint={}",
                                        request_id,
                                        request_strength,
                                        state.preset,
                                        readback.gain,
                                        readback.profile.black_translation,
                                        readback.profile.cross_channel_luma,
                                        readback.child,
                                        readback.source,
                                        readback.refresh_interval_ms,
                                        state.build_fingerprint
                                    );
                                    last_strength = Some(request_strength);
                                    last_mode =
                                        Some((state.preset, state.force_bright, state.prefer_gpu));
                                    last_excluded = Some(readback.excluded.clone());
                                    emit_state(&app, &state);
                                }
                                Err(fallback_error) => {
                                    log::warn!(
                                        "night vision: native magnifier fallback failed: {fallback_error}"
                                    );
                                    let failed = if destroy_magnifier(&app) {
                                        app.state::<NightVision>().controller.confirm_visual_hidden(
                                            Some("night_vision.filter_unavailable"),
                                        )
                                    } else {
                                        app.state::<NightVision>()
                                            .controller
                                            .mark_filter_cleanup_failed()
                                    };
                                    emit_state(&app, &failed);
                                    last_strength = None;
                                    last_excluded = None;
                                }
                            }
                        }
                    }
                }
            }

            if since_topmost >= TOPMOST_MS {
                since_topmost = 0;
                force_overlay_stack();
            }
            effective_previous = true;
        }
    });
}

struct WindowHealth {
    ready: AtomicBool,
    last_signal: Mutex<Instant>,
}

impl WindowHealth {
    fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            last_signal: Mutex::new(Instant::now()),
        }
    }

    fn reset(&self) {
        self.ready.store(false, Ordering::SeqCst);
        *self.last_signal.lock_safe() = Instant::now();
    }

    fn mark_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
        self.mark_heartbeat();
    }

    fn mark_heartbeat(&self) {
        *self.last_signal.lock_safe() = Instant::now();
    }

    fn snapshot(&self) -> (bool, u64) {
        let age = Instant::now().saturating_duration_since(*self.last_signal.lock_safe());
        (
            self.ready.load(Ordering::SeqCst),
            age.as_millis().min(u64::MAX as u128) as u64,
        )
    }
}

pub fn create_button(app: &AppHandle) -> tauri::Result<()> {
    let health = Arc::new(WindowHealth::new());
    let ready_health = health.clone();
    app.listen_any("night-vision://ready", move |_| {
        ready_health.mark_ready();
    });
    let heartbeat_health = health.clone();
    app.listen_any("night-vision://heartbeat", move |_| {
        heartbeat_health.mark_heartbeat();
    });

    build_button_window(app, &health)?;
    spawn_button_supervisor(app.clone(), health);
    Ok(())
}

fn build_button_window(
    app: &AppHandle,
    health: &WindowHealth,
) -> tauri::Result<tauri::WebviewWindow> {
    health.reset();
    let window = WebviewWindowBuilder::new(
        app,
        "night-vision",
        WebviewUrl::App("night-vision.html".into()),
    )
    .title("night vision")
    .inner_size(BUTTON_WIDTH, BUTTON_HEIGHT)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .focusable(false)
    .visible(false)
    .build()?;

    if let Ok(hwnd) = window.hwnd() {
        let raw = hwnd.0 as isize;
        crate::win::vis::register("night-vision", raw);
        crate::win::overlay::assert_overlay_styles(raw);
    }
    let _ = window.set_ignore_cursor_events(false);
    Ok(window)
}

fn button_should_show(show_button: bool, game_active: bool, main_in_front: bool) -> bool {
    show_button && game_active && !main_in_front
}

fn button_needs_recreate(
    ready: bool,
    heartbeat_age_ms: u64,
    visible: bool,
    visible_for_ms: u64,
) -> bool {
    (!ready && heartbeat_age_ms >= 5_000)
        || (ready && visible && visible_for_ms >= 6_000 && heartbeat_age_ms >= 6_000)
}

fn button_anchor(
    game_rect: (i32, i32, i32, i32),
    scale: f64,
    logical_size: (f64, f64),
    logical_margin: f64,
) -> (i32, i32) {
    let (left, top, width, height) = game_rect;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let window_width = (logical_size.0.max(0.0) * scale).round() as i32;
    let window_height = (logical_size.1.max(0.0) * scale).round() as i32;
    let margin = (logical_margin.max(0.0) * scale).round() as i32;
    let max_x = (left + width - window_width).max(left);
    let max_y = (top + height - window_height).max(top);
    let x = (left + width - window_width - margin).clamp(left, max_x);
    let y = (top + margin).clamp(top, max_y);
    (x, y)
}

fn spawn_button_supervisor(app: AppHandle, health: Arc<WindowHealth>) {
    std::thread::spawn(move || {
        const TICK_MS: u64 = 250;
        const GAME_SEARCH_MS: u64 = 1000;
        const RECREATE_MS: u64 = 5000;
        const TOPMOST_MS: u64 = 2000;
        let mut game_hwnd = None;
        let mut since_search = GAME_SEARCH_MS;
        let mut since_recreate = RECREATE_MS;
        let mut since_topmost = 0u64;
        let mut unfocused_ticks: u8 = 2;
        let mut effective_previous = false;
        let mut last_rect = None;
        let mut visible_since: Option<Instant> = None;

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            let Some(window) = app.get_webview_window("night-vision") else {
                since_recreate = since_recreate.saturating_add(TICK_MS);
                if since_recreate >= RECREATE_MS {
                    since_recreate = 0;
                    match build_button_window(&app, &health) {
                        Ok(_) => {
                            effective_previous = false;
                            last_rect = None;
                            visible_since = None;
                        }
                        Err(error) => log::warn!("night vision button recreate failed: {error}"),
                    }
                }
                continue;
            };
            since_recreate = 0;
            since_search = since_search.saturating_add(TICK_MS);
            since_topmost = since_topmost.saturating_add(TICK_MS);

            if since_search >= GAME_SEARCH_MS {
                since_search = 0;
                game_hwnd = crate::win::game_window::find_game_window(GAME_PROCESS_NAME);
            }
            let game_present =
                game_hwnd.is_some_and(|hwnd| !crate::win::game_window::is_iconic(hwnd));
            if game_present && game_hwnd.is_some_and(crate::win::game_window::is_foreground) {
                unfocused_ticks = 0;
            } else {
                unfocused_ticks = unfocused_ticks.saturating_add(1);
            }
            let game_active = game_present && unfocused_ticks < 2;
            let show_button = {
                let state = app.state::<AppState>();
                let settings = state.settings.lock_safe();
                crate::settings::get_bool(&settings, &["night_vision", "show_button"], true)
            };
            let effective = button_should_show(
                show_button,
                game_active,
                crate::win::vis::is_foreground("main"),
            );

            let visible =
                effective_previous && crate::win::vis::is_visible("night-vision") == Some(true);
            if !visible {
                visible_since = None;
            }
            let visible_for_ms = visible_since
                .map(|since| since.elapsed().as_millis().min(u64::MAX as u128) as u64)
                .unwrap_or(0);
            let (ready, heartbeat_age_ms) = health.snapshot();
            if button_needs_recreate(ready, heartbeat_age_ms, visible, visible_for_ms) {
                log::warn!(
                    "night vision button unhealthy (ready={ready}, age={heartbeat_age_ms}ms, visible={visible}); recreating"
                );
                health.reset();
                let _ = window.destroy();
                since_recreate = RECREATE_MS;
                effective_previous = false;
                last_rect = None;
                visible_since = None;
                continue;
            }

            if effective != effective_previous {
                if effective {
                    crate::webview_mem::on_shown(&window);
                    if window.show().is_ok() {
                        effective_previous = true;
                        visible_since = Some(Instant::now());
                        if let Some(hwnd) = crate::win::vis::hwnd("night-vision") {
                            crate::win::overlay::force_topmost(hwnd);
                        }
                        emit_state(&app, &app.state::<NightVision>().controller.state());
                        last_rect = None;
                    }
                } else if window.hide().is_ok() {
                    effective_previous = false;
                    visible_since = None;
                    crate::webview_mem::on_hidden(&window);
                }
            } else if effective && crate::win::vis::is_visible("night-vision") == Some(false) {
                crate::webview_mem::on_shown(&window);
                if window.show().is_ok() {
                    visible_since = Some(Instant::now());
                    if let Some(hwnd) = crate::win::vis::hwnd("night-vision") {
                        crate::win::overlay::force_topmost(hwnd);
                    }
                }
            }

            if !effective_previous {
                continue;
            }
            if let Some(hwnd) = game_hwnd {
                if let Some(rect) = crate::win::game_window::client_rect_on_screen(hwnd) {
                    if last_rect != Some(rect) {
                        last_rect = Some(rect);
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let (x, y) = button_anchor(
                            rect,
                            scale,
                            (BUTTON_WIDTH, BUTTON_HEIGHT),
                            BUTTON_MARGIN,
                        );
                        let _ = window.set_position(PhysicalPosition::new(x, y));
                    }
                }
            }
            if since_topmost >= TOPMOST_MS {
                since_topmost = 0;
                if let Some(hwnd) = crate::win::vis::hwnd("night-vision") {
                    crate::win::overlay::ensure_topmost(hwnd);
                }
            }
        }
    });
}

fn active_game_target() -> Option<GameTarget> {
    let hwnd = crate::win::game_window::find_game_window(GAME_PROCESS_NAME)?;
    if crate::win::game_window::is_iconic(hwnd) || !crate::win::game_window::is_foreground(hwnd) {
        return None;
    }
    match windows::display_name_for_window(hwnd) {
        Ok(display_name) => Some(GameTarget { hwnd, display_name }),
        Err(error) => {
            log::warn!("night vision: could not resolve game display: {error}");
            None
        }
    }
}

fn spawn_supervisor(app: AppHandle) {
    std::thread::spawn(move || {
        const TICK_MS: u64 = 250;
        const GAME_SEARCH_MS: u64 = 1000;
        let mut game_hwnd = None;
        let mut since_search = GAME_SEARCH_MS;
        let mut unfocused_ticks: u8 = 2;
        let mut previous = app.state::<NightVision>().controller.state();

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            since_search = since_search.saturating_add(TICK_MS);
            if since_search >= GAME_SEARCH_MS {
                since_search = 0;
                game_hwnd = crate::win::game_window::find_game_window(GAME_PROCESS_NAME);
            }

            let game_present =
                game_hwnd.is_some_and(|hwnd| !crate::win::game_window::is_iconic(hwnd));
            if game_present && game_hwnd.is_some_and(crate::win::game_window::is_foreground) {
                unfocused_ticks = 0;
            } else {
                unfocused_ticks = unfocused_ticks.saturating_add(1);
            }

            let target = if game_present && unfocused_ticks < 2 {
                game_hwnd.and_then(|hwnd| match windows::display_name_for_window(hwnd) {
                    Ok(display_name) => Some(GameTarget { hwnd, display_name }),
                    Err(error) => {
                        log::warn!("night vision: display lookup failed: {error}");
                        None
                    }
                })
            } else {
                None
            };

            let state = app.state::<NightVision>().controller.reconcile(target);
            if state != previous {
                emit_state(&app, &state);
                previous = state;
            }
        }
    });
}

fn emit_state(app: &AppHandle, state: &NightVisionState) {
    crate::events::emit_all(app, CHANGED_EVENT, state.clone());
}

#[cfg(feature = "devtools")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    display_name: String,
    strength: u8,
    original_samples: [u16; 4],
    applied_samples: [u16; 4],
    restored_samples: [u16; 4],
}

#[cfg(feature = "devtools")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuProbeReport {
    renderer: VisibilityRenderer,
    game_hwnd: isize,
    source: (i32, i32, i32, i32),
    preset: VisibilityPreset,
    presented_frames: u64,
    median_interval_ms: f32,
    presented_fps: f32,
    scene_luma: f32,
    readback_age_ms: u128,
}

#[cfg(feature = "devtools")]
pub fn run_gpu_visibility_probe(
    game_hwnd: isize,
    strength: u8,
    duration_ms: u64,
) -> Result<GpuProbeReport, String> {
    let source = crate::win::game_window::client_rect_on_screen(game_hwnd)
        .filter(|(_, _, width, height)| *width > 0 && *height > 0)
        .ok_or_else(|| "The Isle client rectangle is unavailable".to_string())?;
    let readback = gpu::run_machine_probe(
        game_hwnd,
        source,
        VisibilityPreset::Ultra,
        strength.min(100),
        Duration::from_millis(duration_ms.max(500)),
    )
    .map_err(|error| error.to_string())?;
    let now = Instant::now();
    if !readback.is_current_for(
        VisibilityRenderer::GpuAdaptive,
        game_hwnd,
        source,
        VisibilityPreset::Ultra,
        now,
    ) {
        return Err("GPU probe readback is stale or mismatched".to_string());
    }
    Ok(GpuProbeReport {
        renderer: readback.renderer,
        game_hwnd: readback.game_hwnd,
        source: readback.source,
        preset: readback.preset,
        presented_frames: readback.presented_frames,
        median_interval_ms: readback.median_interval_ms,
        presented_fps: 1000.0 / readback.median_interval_ms,
        scene_luma: readback.scene_luma,
        readback_age_ms: now.duration_since(readback.last_presented_at).as_millis(),
    })
}

#[cfg(feature = "devtools")]
pub fn run_machine_probe(
    game_hwnd: isize,
    strength: u8,
    hold_ms: u64,
) -> Result<ProbeReport, String> {
    let requested = curve::lifted_ramp(strength);
    let mut display = windows::DisplayGamma::for_game_window(
        game_hwnd,
        crate::settings::night_vision_recovery_path(),
    )
    .map_err(|error| error.to_string())?;
    let display_name = display.display_name().to_string();
    let original_samples = samples(display.original());

    display
        .apply_verified(&requested)
        .map_err(|error| error.to_string())?;
    let applied_samples = samples(&display.read_current().map_err(|error| error.to_string())?);
    std::thread::sleep(std::time::Duration::from_millis(hold_ms));
    display.restore().map_err(|error| error.to_string())?;
    let restored_samples = samples(&display.read_current().map_err(|error| error.to_string())?);

    Ok(ProbeReport {
        display_name,
        strength: strength.min(100),
        original_samples,
        applied_samples,
        restored_samples,
    })
}

#[cfg(feature = "devtools")]
fn samples(ramp: &GammaRamp) -> [u16; 4] {
    [ramp[0][32], ramp[0][64], ramp[0][128], ramp[0][255]]
}

#[cfg(test)]
mod tests {
    use super::{
        visibility::{RendererReadback, VisibilityPreset, VisibilityRenderer},
        GameTarget, NightVisionController,
    };
    use std::time::{Duration, Instant};

    fn target(display_name: &str, hwnd: isize) -> GameTarget {
        GameTarget {
            hwnd,
            display_name: display_name.to_string(),
        }
    }

    fn controller() -> NightVisionController {
        NightVisionController::new(70)
    }

    #[test]
    fn overlay_stack_keeps_navigation_above_the_night_filter() {
        assert_eq!(
            super::OVERLAY_STACK_LABELS,
            [
                super::FILTER_LABEL,
                "minimap",
                "hud",
                "water-guide",
                "night-vision",
            ]
        );
        assert!(super::CAPTURE_EXCLUDED_LABELS.contains(&"water-guide"));
    }

    #[test]
    fn visual_boost_gain_has_contrast_preserving_bounds_and_default() {
        assert!((super::visual_boost_gain(0) - 1.0).abs() < f32::EPSILON);
        assert!((super::visual_boost_gain(1) - 1.04).abs() < f32::EPSILON);
        assert!((super::visual_boost_gain(50) - 3.0).abs() < f32::EPSILON);
        assert!((super::visual_boost_gain(70) - 3.8).abs() < f32::EPSILON);
        assert!((super::visual_boost_gain(100) - 5.0).abs() < f32::EPSILON);
        assert!((super::visual_boost_gain(u8::MAX) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn visual_boost_gain_is_monotonic_for_every_strength_step() {
        for strength in 0..100 {
            assert!(
                super::visual_boost_gain(strength + 1) > super::visual_boost_gain(strength),
                "gain must increase between {strength} and {}",
                strength + 1
            );
        }
    }

    #[test]
    fn native_visual_readback_and_visibility_are_required_to_turn_boost_on() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();

        assert!(
            !controller
                .accept_native_visual(request_id - 1, 70, true, true)
                .applied
        );
        assert!(
            !controller
                .accept_native_visual(request_id, 70, false, true)
                .applied
        );
        assert!(
            !controller
                .accept_native_visual(request_id, 70, true, false)
                .applied
        );
        assert!(
            controller
                .accept_native_visual(request_id, 70, true, true)
                .applied
        );
    }

    #[test]
    fn stale_or_mismatched_gpu_readback_never_turns_boost_on() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, strength) = controller.begin_visual_request().unwrap();
        let now = Instant::now();
        let expected_source = (100, 200, 1920, 1080);
        let readback = RendererReadback {
            renderer: VisibilityRenderer::GpuAdaptive,
            game_hwnd: 101,
            source: expected_source,
            preset: VisibilityPreset::Ultra,
            presented_frames: 120,
            last_presented_at: now - Duration::from_millis(100),
            median_interval_ms: 16.7,
            scene_luma: 0.03,
        };

        let mut stale = readback;
        stale.last_presented_at = now - Duration::from_millis(501);
        assert!(
            !controller
                .accept_renderer_readback(
                    request_id,
                    strength,
                    101,
                    expected_source,
                    stale,
                    now,
                    true,
                )
                .applied
        );

        let mut wrong_target = readback;
        wrong_target.game_hwnd = 999;
        assert!(
            !controller
                .accept_renderer_readback(
                    request_id,
                    strength,
                    101,
                    expected_source,
                    wrong_target,
                    now,
                    true,
                )
                .applied
        );

        let applied = controller.accept_renderer_readback(
            request_id,
            strength,
            101,
            expected_source,
            readback,
            now,
            true,
        );
        assert!(applied.applied && applied.visual_boost_applied);
        assert_eq!(applied.renderer, VisibilityRenderer::GpuAdaptive);
        assert_eq!(applied.preset, VisibilityPreset::Ultra);
        assert_eq!(applied.scene_luma, Some(0.03));
        assert_eq!(applied.presented_fps, Some(1000.0 / 16.7));
    }

    #[test]
    fn active_gpu_readback_refreshes_metrics_but_rejects_stale_or_wrong_target_frames() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, strength) = controller.begin_visual_request().unwrap();
        let source = (100, 200, 1920, 1080);
        let start = Instant::now();
        let first = RendererReadback {
            renderer: VisibilityRenderer::GpuAdaptive,
            game_hwnd: 101,
            source,
            preset: VisibilityPreset::Ultra,
            presented_frames: 2,
            last_presented_at: start,
            median_interval_ms: 20.0,
            scene_luma: 0.04,
        };
        assert!(
            controller
                .accept_renderer_readback(request_id, strength, 101, source, first, start, true)
                .applied
        );

        let now = start + Duration::from_millis(100);
        let current = RendererReadback {
            presented_frames: 9,
            last_presented_at: now,
            median_interval_ms: 10.0,
            scene_luma: 0.02,
            ..first
        };
        let refreshed = controller
            .refresh_renderer_readback(101, source, current, now, true)
            .expect("current GPU frame must refresh truthful metrics");
        assert_eq!(refreshed.scene_luma, Some(0.02));
        assert_eq!(refreshed.presented_fps, Some(100.0));

        let stale = RendererReadback {
            last_presented_at: now - Duration::from_millis(501),
            ..current
        };
        assert!(controller
            .refresh_renderer_readback(101, source, stale, now, true)
            .is_none());
        assert!(controller
            .refresh_renderer_readback(999, source, current, now, true)
            .is_none());
    }

    #[test]
    fn normal_operation_never_applies_display_gamma() {
        let controller = controller();
        controller.toggle_requested();

        let state = controller.reconcile(Some(target("DISPLAY1", 101)));

        assert!(!state.gamma_applied);
        assert!(!state.applied);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.filter_unavailable")
        );
    }

    #[test]
    fn ready_native_visual_path_never_reports_gamma() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        controller.reconcile(Some(target("DISPLAY1", 101)));
        let (request_id, _) = controller.begin_visual_request().unwrap();

        let state = controller.accept_native_visual(request_id, 70, true, true);

        assert!(state.applied && state.visual_boost_applied);
        assert!(!state.gamma_applied);
        assert!(state.supported);
    }

    #[test]
    fn filter_visibility_requires_request_ready_and_foreground_game() {
        assert!(super::filter_should_show(true, true, true, false));
        assert!(!super::filter_should_show(false, true, true, false));
        assert!(!super::filter_should_show(true, false, true, false));
        assert!(!super::filter_should_show(true, true, false, false));
        assert!(!super::filter_should_show(true, true, true, true));
    }

    #[test]
    fn hidden_filter_clears_visual_only_after_cleanup_is_confirmed() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();
        controller.accept_native_visual(request_id, 70, true, true);

        let state = controller.confirm_visual_hidden(Some("night_vision.waiting_for_game"));

        assert!(state.requested);
        assert!(!state.applied && !state.visual_boost_applied);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.waiting_for_game")
        );
    }

    #[test]
    fn startup_is_off_and_active_game_waits_for_verified_native_visual() {
        let controller = controller();
        assert!(!controller.state().requested);
        assert!(!controller.state().applied);
        assert!(controller.state().supported);

        controller.toggle_requested();
        let state = controller.reconcile(Some(target("DISPLAY1", 101)));

        assert!(state.requested);
        assert!(!state.applied);
        assert!(!state.visual_boost_applied);
        assert!(!state.gamma_applied);
        assert!(!state.supported);
        assert_eq!(state.strength, 70);
    }

    #[test]
    fn alt_tab_preserves_request_and_waits_for_verified_cleanup() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();
        controller.accept_native_visual(request_id, 70, true, true);

        let away = controller.reconcile(None);
        assert!(away.requested);
        assert!(
            away.applied,
            "state stays on until native cleanup is verified"
        );

        let hidden = controller.confirm_visual_hidden(Some("night_vision.waiting_for_game"));
        assert!(hidden.requested);
        assert!(!hidden.applied);
        assert_eq!(
            hidden.error_key.as_deref(),
            Some("night_vision.waiting_for_game")
        );

        let back = controller.reconcile(Some(target("DISPLAY1", 101)));
        assert!(back.requested && !back.gamma_applied);
        assert!(!back.applied);
        assert!(controller.begin_visual_request().is_some());
    }

    #[test]
    fn switch_off_is_fail_closed_until_native_cleanup_succeeds() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();
        controller.accept_native_visual(request_id, 70, true, true);

        let switching_off = controller.toggle_requested();
        assert!(!switching_off.requested);
        assert!(switching_off.applied && switching_off.visual_boost_applied);

        let failed = controller.mark_filter_cleanup_failed();
        assert!(!failed.requested);
        assert!(failed.applied && failed.visual_boost_applied);
        assert_eq!(
            failed.error_key.as_deref(),
            Some("night_vision.filter_cleanup_error")
        );

        let cleaned = controller.confirm_visual_hidden(None);
        assert!(!cleaned.requested);
        assert!(!cleaned.applied && !cleaned.visual_boost_applied);
        assert_eq!(cleaned.error_key, None);
    }

    #[test]
    fn strength_change_keeps_old_effect_truthful_until_destroy_then_reapplies() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();
        controller.accept_native_visual(request_id, 70, true, true);

        let changing = controller.set_strength(u8::MAX);
        assert_eq!(changing.strength, 100);
        assert!(changing.applied, "the old native effect still exists");

        let hidden = controller.confirm_visual_hidden(None);
        assert!(!hidden.applied);
        let (new_request, _) = controller.begin_visual_request().unwrap();
        let reapplied = controller.accept_native_visual(new_request, 100, true, true);
        assert!(reapplied.applied);
        assert_eq!(reapplied.strength, 100);
        assert!(!reapplied.gamma_applied);
    }

    #[test]
    fn visibility_mode_changes_cancel_pending_work_and_preserve_truth_until_cleanup() {
        use std::sync::atomic::Ordering;

        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();
        assert_eq!(
            controller.native_generation.load(Ordering::SeqCst),
            request_id
        );

        let preset = controller.set_preset(VisibilityPreset::Clear);
        assert_eq!(preset.preset, VisibilityPreset::Clear);
        assert_eq!(controller.native_generation.load(Ordering::SeqCst), 0);

        let force = controller.set_force_bright(false);
        assert!(!force.force_bright);
        let gpu = controller.set_prefer_gpu(false);
        assert!(!gpu.prefer_gpu);
        assert!(controller.begin_visual_request().is_some());
    }

    #[test]
    fn off_and_strength_changes_cancel_delayed_native_work_immediately() {
        use std::sync::atomic::Ordering;

        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (first_request, _) = controller.begin_visual_request().unwrap();
        assert_eq!(
            controller.native_generation.load(Ordering::SeqCst),
            first_request
        );
        controller.set_strength(80);
        assert_eq!(controller.native_generation.load(Ordering::SeqCst), 0);

        let (second_request, strength) = controller.begin_visual_request().unwrap();
        assert_eq!(strength, 80);
        assert_eq!(
            controller.native_generation.load(Ordering::SeqCst),
            second_request
        );
        controller.toggle_requested();
        assert_eq!(controller.native_generation.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unresolved_crash_recovery_blocks_enabling_over_unknown_gamma() {
        let controller = controller();

        controller.block_for_recovery_error();
        controller.toggle_requested();
        let state = controller.reconcile(Some(target("DISPLAY1", 101)));

        assert!(!state.requested);
        assert!(!state.applied);
        assert!(!state.supported);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.recovery_error")
        );
    }

    #[test]
    fn exit_state_is_fail_closed_until_native_cleanup_is_verified() {
        let controller = controller();
        controller.toggle_requested();
        controller.mark_filter_ready();
        let (request_id, _) = controller.begin_visual_request().unwrap();
        controller.accept_native_visual(request_id, 70, true, true);

        let blocked = controller.finish_exit(false);
        assert!(!blocked.requested);
        assert!(blocked.applied && blocked.visual_boost_applied);
        assert_eq!(
            blocked.error_key.as_deref(),
            Some("night_vision.filter_cleanup_error")
        );

        let safe = controller.finish_exit(true);
        assert!(!safe.applied && !safe.visual_boost_applied && !safe.gamma_applied);
    }

    #[test]
    fn button_visibility_requires_user_setting_and_foreground_game() {
        assert!(super::button_should_show(true, true, false));
        assert!(!super::button_should_show(false, true, false));
        assert!(!super::button_should_show(true, false, false));
        assert!(!super::button_should_show(true, true, true));
    }

    #[test]
    fn button_anchor_stays_inside_game_top_right_at_display_scale() {
        let rect = (100, 200, 1920, 1080);
        let (x, y) = super::button_anchor(
            rect,
            1.5,
            (super::BUTTON_WIDTH, super::BUTTON_HEIGHT),
            super::BUTTON_MARGIN,
        );

        assert_eq!((x, y), (1711, 224));
        assert!(x >= rect.0);
        assert!(y >= rect.1);
        assert!(x + (super::BUTTON_WIDTH * 1.5).round() as i32 <= rect.0 + rect.2);
        assert!(y + (super::BUTTON_HEIGHT * 1.5).round() as i32 <= rect.1 + rect.3);
    }

    #[test]
    fn button_health_recreates_unready_or_visible_stale_webviews_only() {
        assert!(super::button_needs_recreate(false, 5_000, false, 0));
        assert!(!super::button_needs_recreate(false, 4_999, true, 9_000));
        assert!(super::button_needs_recreate(true, 6_000, true, 6_000));
        assert!(!super::button_needs_recreate(true, 60_000, false, 60_000));
        assert!(!super::button_needs_recreate(true, 6_000, true, 5_999));
    }
}
