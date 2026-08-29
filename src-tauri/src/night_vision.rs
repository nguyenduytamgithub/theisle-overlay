mod curve;
mod recovery;
mod windows;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Listener, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::settings::GAME_PROCESS_NAME;
use crate::state::AppState;
use crate::state::LockExt;

pub(crate) use curve::GammaRamp;
pub(crate) use windows::NightVisionError;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GameTarget {
    pub(crate) hwnd: isize,
    pub(crate) display_name: String,
}

pub const CHANGED_EVENT: &str = "night-vision://changed";
pub const BUILD_FINGERPRINT: &str = concat!(env!("CARGO_PKG_VERSION"), "-visual-boost-a");
const FILTER_LABEL: &str = "night-vision-filter";
const FILTER_READY_EVENT: &str = "night-vision-filter://ready";
const FILTER_HEARTBEAT_EVENT: &str = "night-vision-filter://heartbeat";
const FILTER_PAINT_EVENT: &str = "night-vision-filter://paint";
const FILTER_PAINTED_EVENT: &str = "night-vision-filter://painted";
const FILTER_COLOR: &str = "rgb(235, 240, 230)";
const BUTTON_WIDTH: f64 = 164.0;
const BUTTON_HEIGHT: f64 = 42.0;
const BUTTON_MARGIN: f64 = 12.0;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
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
    pub build_fingerprint: &'static str,
}

pub(crate) fn visual_boost_alpha(strength: u8) -> f64 {
    let strength = strength.min(100);
    if strength == 0 {
        0.0
    } else {
        0.05 + 0.0025 * f64::from(strength)
    }
}

pub(crate) trait GammaSession: Send {
    fn display_name(&self) -> &str;
    fn apply(&mut self, ramp: &GammaRamp) -> Result<(), NightVisionError>;
    fn restore(&mut self) -> Result<(), NightVisionError>;
}

impl GammaSession for windows::DisplayGamma {
    fn display_name(&self) -> &str {
        self.display_name()
    }

    fn apply(&mut self, ramp: &GammaRamp) -> Result<(), NightVisionError> {
        self.apply_verified(ramp)
    }

    fn restore(&mut self) -> Result<(), NightVisionError> {
        windows::DisplayGamma::restore(self)
    }
}

pub(crate) trait DisplayFactory: Send + Sync {
    type Session: GammaSession;

    fn open(
        &self,
        target: &GameTarget,
        recovery_path: &Path,
    ) -> Result<Self::Session, NightVisionError>;
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Win32DisplayFactory;

impl DisplayFactory for Win32DisplayFactory {
    type Session = windows::DisplayGamma;

    fn open(
        &self,
        target: &GameTarget,
        recovery_path: &Path,
    ) -> Result<Self::Session, NightVisionError> {
        windows::DisplayGamma::for_game_window(target.hwnd, recovery_path.to_path_buf())
    }
}

struct ControllerInner<S: GammaSession> {
    state: NightVisionState,
    session: Option<S>,
    applied_strength: Option<u8>,
    gamma_supported: bool,
    recovery_blocked: bool,
    restore_pending: bool,
    next_visual_request: u64,
    pending_visual_request: Option<(u64, u8)>,
}

pub(crate) struct NightVisionController<F: DisplayFactory> {
    factory: F,
    recovery_path: PathBuf,
    inner: Mutex<ControllerInner<F::Session>>,
}

impl<F: DisplayFactory> NightVisionController<F> {
    pub(crate) fn new(factory: F, recovery_path: PathBuf, strength: u8) -> Self {
        Self {
            factory,
            recovery_path,
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
                    build_fingerprint: BUILD_FINGERPRINT,
                },
                session: None,
                applied_strength: None,
                gamma_supported: true,
                recovery_blocked: false,
                restore_pending: false,
                next_visual_request: 0,
                pending_visual_request: None,
            }),
        }
    }

    pub(crate) fn state(&self) -> NightVisionState {
        self.inner.lock_safe().state.clone()
    }

    pub(crate) fn toggle_requested(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        if inner.recovery_blocked {
            return inner.state.clone();
        }
        inner.state.requested = !inner.state.requested;
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        inner.pending_visual_request = None;
        if !inner.restore_pending {
            inner.state.error_key = None;
            if inner.state.requested {
                inner.state.supported = true;
                inner.gamma_supported = true;
            }
        }
        inner.state.clone()
    }

    pub(crate) fn block_for_recovery_error(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.recovery_blocked = true;
        inner.restore_pending = false;
        inner.state.requested = false;
        inner.state.applied = false;
        inner.state.visual_boost_applied = false;
        inner.state.gamma_applied = false;
        inner.state.supported = false;
        inner.state.error_key = Some("night_vision.recovery_error".to_string());
        inner.state.clone()
    }

    pub(crate) fn set_strength(&self, strength: u8) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        let strength = strength.min(100);
        if inner.state.strength != strength {
            inner.state.strength = strength;
            inner.applied_strength = None;
            inner.state.visual_boost_applied = false;
            inner.state.applied = false;
            inner.pending_visual_request = None;
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

    pub(crate) fn begin_visual_request(&self) -> Option<u64> {
        let mut inner = self.inner.lock_safe();
        if inner.recovery_blocked || !inner.state.requested || !inner.state.visual_boost_ready {
            return None;
        }
        if let Some((request_id, strength)) = inner.pending_visual_request {
            if strength == inner.state.strength {
                return Some(request_id);
            }
        }
        inner.next_visual_request = inner.next_visual_request.wrapping_add(1).max(1);
        let request_id = inner.next_visual_request;
        let strength = inner.state.strength;
        inner.pending_visual_request = Some((request_id, strength));
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        Some(request_id)
    }

    pub(crate) fn accept_visual_paint(
        &self,
        request_id: u64,
        strength: u8,
        window_visible: bool,
    ) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        let expected = inner.pending_visual_request;
        if expected == Some((request_id, strength.min(100)))
            && inner.state.requested
            && inner.state.visual_boost_ready
            && window_visible
        {
            inner.pending_visual_request = None;
            inner.state.visual_boost_applied = true;
            inner.state.applied = true;
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

    pub(crate) fn clear_visual_applied(&self, error_key: &str) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.pending_visual_request = None;
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        if inner.state.requested {
            inner.state.error_key = Some(error_key.to_string());
        }
        inner.state.clone()
    }

    pub(crate) fn mark_filter_failed(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.pending_visual_request = None;
        inner.state.visual_boost_ready = false;
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        inner.state.supported = false;
        inner.state.error_key = Some("night_vision.filter_unavailable".to_string());
        inner.state.clone()
    }

    pub(crate) fn mark_filter_cleanup_failed(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.state.requested = false;
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
            if restore_session(&mut inner) {
                inner.state.gamma_applied = false;
                inner.state.error_key = None;
            }
            return inner.state.clone();
        }

        let Some(target) = game else {
            if restore_session(&mut inner) {
                inner.state.gamma_applied = false;
                inner.state.error_key = Some("night_vision.waiting_for_game".to_string());
            }
            return inner.state.clone();
        };

        let changed_display = inner
            .session
            .as_ref()
            .is_some_and(|session| session.display_name() != target.display_name);
        if (changed_display || inner.restore_pending) && !restore_session(&mut inner) {
            return inner.state.clone();
        }

        if !inner.gamma_supported {
            return inner.state.clone();
        }

        if inner.session.is_none() {
            match self.factory.open(&target, &self.recovery_path) {
                Ok(session) => inner.session = Some(session),
                Err(error) => {
                    mark_failed(&mut inner, &error);
                    return inner.state.clone();
                }
            }
        }

        let strength = inner.state.strength;
        if !inner.state.gamma_applied || inner.applied_strength != Some(strength) {
            let ramp = curve::lifted_ramp(strength);
            let result = inner
                .session
                .as_mut()
                .expect("session exists after factory open")
                .apply(&ramp);
            match result {
                Ok(()) => {
                    inner.state.gamma_applied = true;
                    inner.gamma_supported = true;
                    if inner.state.visual_boost_ready {
                        inner.state.supported = true;
                    }
                    if !inner.state.visual_boost_applied {
                        inner.state.error_key = None;
                    }
                    inner.applied_strength = Some(strength);
                }
                Err(error) => {
                    if restore_session(&mut inner) {
                        mark_failed(&mut inner, &error);
                    }
                }
            }
        }

        inner.state.clone()
    }

    pub(crate) fn restore_for_exit(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.state.requested = false;
        inner.pending_visual_request = None;
        inner.state.visual_boost_applied = false;
        inner.state.applied = false;
        if restore_session(&mut inner) {
            inner.state.gamma_applied = false;
            inner.state.error_key = None;
        } else {
            // `applied` is also the normal-exit safety gate. A gamma ramp that
            // could not be restored must keep shutdown/relaunch blocked.
            inner.state.applied = true;
        }
        inner.state.clone()
    }
}

pub struct NightVision {
    controller: NightVisionController<Win32DisplayFactory>,
}

impl Default for NightVision {
    fn default() -> Self {
        Self::new()
    }
}

impl NightVision {
    pub fn new() -> Self {
        Self {
            controller: NightVisionController::new(
                Win32DisplayFactory,
                crate::settings::night_vision_recovery_path(),
                70,
            ),
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

pub fn toggle_from_app(app: &AppHandle) {
    let night_vision = app.state::<NightVision>();
    night_vision.controller.toggle_requested();
    let state = night_vision.controller.reconcile(active_game_target());
    emit_state(app, &state);
}

pub fn initialize(app: &AppHandle) {
    let night_vision = app.state::<NightVision>();
    let strength = {
        let app_state = app.state::<AppState>();
        let settings = app_state.settings.lock_safe();
        crate::settings::get_f64(&settings, &["night_vision", "strength"], 70.0)
            .round()
            .clamp(0.0, 100.0) as u8
    };
    night_vision.controller.set_strength(strength);

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
    let mut state = night_vision.controller.restore_for_exit();
    if !filter_hidden {
        state = night_vision.controller.mark_filter_cleanup_failed();
        log::error!("night vision: visual filter hide on exit was not verified");
    } else if state.gamma_applied {
        log::error!("night vision: gamma restore on exit was not verified");
    }
    emit_state(app, &state);
    state
}

#[tauri::command]
pub fn prepare_night_vision_exit(app: AppHandle) -> NightVisionState {
    restore_before_exit(&app)
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FilterPaintRequest {
    request_id: u64,
    strength: u8,
    alpha: f64,
    color: &'static str,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterPainted {
    request_id: u64,
    strength: u8,
}

pub fn create_filter(app: &AppHandle) -> tauri::Result<()> {
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

    let painted_app = app.clone();
    app.listen_any(FILTER_PAINTED_EVENT, move |event| {
        let payload = match serde_json::from_str::<FilterPainted>(event.payload()) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("night vision filter sent an invalid painted ack: {error}");
                return;
            }
        };
        let visible = crate::win::vis::is_visible(FILTER_LABEL) == Some(true);
        let state = painted_app
            .state::<NightVision>()
            .controller
            .accept_visual_paint(payload.request_id, payload.strength, visible);
        if state.visual_boost_applied {
            log::info!(
                "night vision: visual boost painted request={} strength={} fingerprint={}",
                payload.request_id,
                payload.strength,
                state.build_fingerprint
            );
        }
        emit_state(&painted_app, &state);
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
        return crate::win::vis::is_visible(FILTER_LABEL) != Some(true);
    };
    crate::webview_mem::on_hidden(&window);
    window.hide().is_ok() && wait_filter_hidden(250)
}

fn force_overlay_stack() {
    for label in [FILTER_LABEL, "minimap", "hud", "night-vision"] {
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
        const PAINT_RETRY_MS: u64 = 1000;
        const TOPMOST_MS: u64 = 2000;

        let mut game_hwnd = None;
        let mut since_search = GAME_SEARCH_MS;
        let mut since_recreate = RECREATE_MS;
        let mut since_paint = PAINT_RETRY_MS;
        let mut since_topmost = TOPMOST_MS;
        let mut unfocused_ticks: u8 = 2;
        let mut effective_previous = false;
        let mut last_rect = None;
        let mut last_strength = None;

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            since_search = since_search.saturating_add(TICK_MS);
            since_recreate = since_recreate.saturating_add(TICK_MS);
            since_paint = since_paint.saturating_add(TICK_MS);
            since_topmost = since_topmost.saturating_add(TICK_MS);

            let Some(window) = app.get_webview_window(FILTER_LABEL) else {
                let state = app
                    .state::<NightVision>()
                    .controller
                    .mark_filter_failed();
                emit_state(&app, &state);
                if since_recreate >= RECREATE_MS {
                    since_recreate = 0;
                    match build_filter_window(&app, &health) {
                        Ok(_) => {
                            effective_previous = false;
                            last_rect = None;
                            last_strength = None;
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
                let _ = window.close();
                let failed = app
                    .state::<NightVision>()
                    .controller
                    .mark_filter_failed();
                emit_state(&app, &failed);
                continue;
            }

            if !effective {
                if crate::win::vis::is_visible(FILTER_LABEL) == Some(true) {
                    hide_filter_window(&app);
                }
                if effective_previous || state.visual_boost_applied {
                    let hidden = app
                        .state::<NightVision>()
                        .controller
                        .clear_visual_applied("night_vision.waiting_for_game");
                    emit_state(&app, &hidden);
                }
                effective_previous = false;
                last_rect = None;
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
                    .and_then(|_| {
                        window.set_size(PhysicalSize::new(width as u32, height as u32))
                    })
                    .is_err()
                {
                    let failed = app
                        .state::<NightVision>()
                        .controller
                        .mark_filter_failed();
                    emit_state(&app, &failed);
                    continue;
                }
                last_rect = Some(rect);
                since_paint = PAINT_RETRY_MS;
            }

            if crate::win::vis::is_visible(FILTER_LABEL) != Some(true) {
                crate::webview_mem::on_shown(&window);
                if window.show().is_err() || !crate::win::vis::wait_visible(FILTER_LABEL, 500) {
                    let failed = app
                        .state::<NightVision>()
                        .controller
                        .mark_filter_failed();
                    emit_state(&app, &failed);
                    continue;
                }
                force_overlay_stack();
                since_paint = PAINT_RETRY_MS;
            }

            let current = app.state::<NightVision>().controller.state();
            if (!current.visual_boost_applied || last_strength != Some(current.strength))
                && since_paint >= PAINT_RETRY_MS
            {
                since_paint = 0;
                if let Some(request_id) = app
                    .state::<NightVision>()
                    .controller
                    .begin_visual_request()
                {
                    let request = FilterPaintRequest {
                        request_id,
                        strength: current.strength,
                        alpha: visual_boost_alpha(current.strength),
                        color: FILTER_COLOR,
                    };
                    crate::events::emit_all(&app, FILTER_PAINT_EVENT, request);
                    last_strength = Some(current.strength);
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

fn restore_session<S: GammaSession>(inner: &mut ControllerInner<S>) -> bool {
    if let Some(session) = inner.session.as_mut() {
        match session.restore() {
            Ok(()) => {}
            Err(NightVisionError::RecoveryCleanup(error)) => {
                // Gamma readback already proved the original ramp is active.
                // A stale recovery file is safe and will be retried at startup;
                // it must never be reported as a still-brightened display.
                log::warn!("night vision recovery cleanup pending: {error}");
            }
            Err(error) => {
                // The original ramp was not verified. Keep both the session and
                // the truthful "display still modified/unknown" state so the
                // next supervisor tick can retry instead of silently abandoning
                // the only in-process restore handle.
                inner.restore_pending = true;
                inner.state.gamma_applied = true;
                inner.gamma_supported = false;
                inner.state.supported = false;
                inner.state.error_key = Some(error_key(&error).to_string());
                return false;
            }
        }
        inner.gamma_supported = true;
    }
    inner.restore_pending = false;
    inner.session = None;
    inner.state.gamma_applied = false;
    inner.applied_strength = None;
    true
}

fn mark_failed<S: GammaSession>(inner: &mut ControllerInner<S>, error: &NightVisionError) {
    inner.restore_pending = false;
    inner.state.gamma_applied = false;
    inner.gamma_supported = false;
    inner.state.supported = inner.state.visual_boost_ready;
    inner.state.error_key = Some(error_key(error).to_string());
    inner.applied_strength = None;
}

fn error_key(error: &NightVisionError) -> &'static str {
    match error {
        NightVisionError::Recovery(_) | NightVisionError::RecoveryCleanup(_) => {
            "night_vision.recovery_error"
        }
        NightVisionError::ReadbackRejected | NightVisionError::RestoreRejected => {
            "night_vision.driver_rejected"
        }
        NightVisionError::Driver(_) => "night_vision.driver_error",
    }
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
        DisplayFactory, GameTarget, GammaSession, NightVisionController, NightVisionError,
    };
    use crate::night_vision::GammaRamp;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Trace {
        operations: Vec<String>,
    }

    #[derive(Clone)]
    struct FakeFactory {
        trace: Arc<Mutex<Trace>>,
        fail_apply: bool,
        fail_restore: bool,
        fail_recovery_cleanup: bool,
    }

    struct FakeSession {
        display_name: String,
        trace: Arc<Mutex<Trace>>,
        fail_apply: bool,
        fail_restore: bool,
        fail_recovery_cleanup: bool,
    }

    impl DisplayFactory for FakeFactory {
        type Session = FakeSession;

        fn open(
            &self,
            target: &GameTarget,
            _recovery_path: &Path,
        ) -> Result<Self::Session, NightVisionError> {
            self.trace
                .lock()
                .unwrap()
                .operations
                .push(format!("open:{}", target.display_name));
            Ok(FakeSession {
                display_name: target.display_name.clone(),
                trace: self.trace.clone(),
                fail_apply: self.fail_apply,
                fail_restore: self.fail_restore,
                fail_recovery_cleanup: self.fail_recovery_cleanup,
            })
        }
    }

    impl GammaSession for FakeSession {
        fn display_name(&self) -> &str {
            &self.display_name
        }

        fn apply(&mut self, ramp: &GammaRamp) -> Result<(), NightVisionError> {
            self.trace
                .lock()
                .unwrap()
                .operations
                .push(format!("apply:{}:{}", self.display_name, ramp[0][128]));
            if self.fail_apply {
                Err(NightVisionError::Driver("fake apply failed".to_string()))
            } else {
                Ok(())
            }
        }

        fn restore(&mut self) -> Result<(), NightVisionError> {
            self.trace
                .lock()
                .unwrap()
                .operations
                .push(format!("restore:{}", self.display_name));
            if self.fail_restore {
                Err(NightVisionError::RestoreRejected)
            } else if self.fail_recovery_cleanup {
                Err(NightVisionError::RecoveryCleanup(
                    "fake cleanup failed".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    }

    fn target(display_name: &str, hwnd: isize) -> GameTarget {
        GameTarget {
            hwnd,
            display_name: display_name.to_string(),
        }
    }

    fn controller(fail_apply: bool) -> (NightVisionController<FakeFactory>, Arc<Mutex<Trace>>) {
        let trace = Arc::new(Mutex::new(Trace::default()));
        let factory = FakeFactory {
            trace: trace.clone(),
            fail_apply,
            fail_restore: false,
            fail_recovery_cleanup: false,
        };
        (
            NightVisionController::new(
                factory,
                std::env::temp_dir().join("unused-night-vision-recovery.json"),
                70,
            ),
            trace,
        )
    }

    #[test]
    fn visual_boost_alpha_has_the_approved_bounds_and_default() {
        assert_eq!(super::visual_boost_alpha(0), 0.0);
        assert!((super::visual_boost_alpha(1) - 0.0525).abs() < f64::EPSILON);
        assert!((super::visual_boost_alpha(70) - 0.225).abs() < f64::EPSILON);
        assert!((super::visual_boost_alpha(100) - 0.30).abs() < f64::EPSILON);
        assert!((super::visual_boost_alpha(u8::MAX) - 0.30).abs() < f64::EPSILON);
    }

    #[test]
    fn stale_paint_ack_never_turns_visual_boost_on() {
        let (controller, _) = controller(false);
        controller.toggle_requested();
        controller.mark_filter_ready();
        let request_id = controller.begin_visual_request().unwrap();

        assert!(
            !controller
                .accept_visual_paint(request_id - 1, 70, true)
                .applied
        );
        assert!(controller.accept_visual_paint(request_id, 70, true).applied);
    }

    #[test]
    fn gamma_failure_does_not_disable_ready_visual_fallback() {
        let (controller, _) = controller(true);
        controller.toggle_requested();
        controller.mark_filter_ready();
        controller.reconcile(Some(target("DISPLAY1", 101)));
        let request_id = controller.begin_visual_request().unwrap();

        let state = controller.accept_visual_paint(request_id, 70, true);

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
    fn hidden_filter_clears_visual_applied_but_preserves_request() {
        let (controller, _) = controller(false);
        controller.toggle_requested();
        controller.mark_filter_ready();
        let request_id = controller.begin_visual_request().unwrap();
        controller.accept_visual_paint(request_id, 70, true);

        let state = controller.clear_visual_applied("night_vision.waiting_for_game");

        assert!(state.requested);
        assert!(!state.applied && !state.visual_boost_applied);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.waiting_for_game")
        );
    }

    #[test]
    fn startup_is_off_and_active_game_toggle_applies_strength_70() {
        let (controller, trace) = controller(false);
        assert!(!controller.state().requested);
        assert!(!controller.state().applied);
        assert!(controller.state().supported);

        controller.toggle_requested();
        let state = controller.reconcile(Some(target("DISPLAY1", 101)));

        assert!(state.requested);
        assert!(!state.applied);
        assert!(!state.visual_boost_applied);
        assert!(state.gamma_applied);
        assert!(state.supported);
        assert_eq!(state.strength, 70);
        assert_eq!(trace.lock().unwrap().operations.len(), 2);
    }

    #[test]
    fn alt_tab_restores_but_keeps_request_and_refocus_reapplies() {
        let (controller, trace) = controller(false);
        controller.toggle_requested();
        controller.reconcile(Some(target("DISPLAY1", 101)));

        let away = controller.reconcile(None);
        assert!(away.requested);
        assert!(!away.applied);
        assert_eq!(
            away.error_key.as_deref(),
            Some("night_vision.waiting_for_game")
        );

        let back = controller.reconcile(Some(target("DISPLAY1", 101)));
        assert!(back.requested && back.gamma_applied);
        assert!(!back.applied);
        assert_eq!(
            trace.lock().unwrap().operations,
            vec![
                "open:DISPLAY1".to_string(),
                format!("apply:DISPLAY1:{}", super::curve::lifted_ramp(70)[0][128]),
                "restore:DISPLAY1".to_string(),
                "open:DISPLAY1".to_string(),
                format!("apply:DISPLAY1:{}", super::curve::lifted_ramp(70)[0][128]),
            ]
        );
    }

    #[test]
    fn monitor_switch_restores_old_display_before_opening_new_display() {
        let (controller, trace) = controller(false);
        controller.toggle_requested();
        controller.reconcile(Some(target("DISPLAY1", 101)));
        controller.reconcile(Some(target("DISPLAY2", 101)));

        let operations = &trace.lock().unwrap().operations;
        let restore_index = operations
            .iter()
            .position(|entry| entry == "restore:DISPLAY1")
            .unwrap();
        let open_index = operations
            .iter()
            .position(|entry| entry == "open:DISPLAY2")
            .unwrap();
        assert!(restore_index < open_index);
        assert!(controller.state().gamma_applied);
        assert!(!controller.state().applied);
    }

    #[test]
    fn monitor_switch_does_not_touch_new_display_when_old_restore_fails() {
        let trace = Arc::new(Mutex::new(Trace::default()));
        let controller = NightVisionController::new(
            FakeFactory {
                trace: trace.clone(),
                fail_apply: false,
                fail_restore: true,
                fail_recovery_cleanup: false,
            },
            std::env::temp_dir().join("unused-night-vision-recovery.json"),
            70,
        );
        controller.toggle_requested();
        controller.reconcile(Some(target("DISPLAY1", 101)));

        let state = controller.reconcile(Some(target("DISPLAY2", 202)));

        assert!(state.gamma_applied);
        assert!(!state.applied);
        assert!(!state.supported);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.driver_rejected")
        );
        assert!(
            !trace
                .lock()
                .unwrap()
                .operations
                .iter()
                .any(|operation| operation == "open:DISPLAY2"),
            "new display must stay untouched until old gamma restore is verified"
        );
    }

    #[test]
    fn rejected_driver_state_is_truthfully_unavailable() {
        let (controller, _trace) = controller(true);
        controller.toggle_requested();

        let state = controller.reconcile(Some(target("DISPLAY1", 101)));

        assert!(!state.applied);
        assert!(!state.supported);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.driver_error")
        );
    }

    #[test]
    fn strength_is_clamped_and_reapplied_while_active() {
        let (controller, trace) = controller(false);
        controller.toggle_requested();
        let game = target("DISPLAY1", 101);
        controller.reconcile(Some(game.clone()));

        controller.set_strength(u8::MAX);
        let state = controller.reconcile(Some(game));

        assert_eq!(state.strength, 100);
        let apply_count = trace
            .lock()
            .unwrap()
            .operations
            .iter()
            .filter(|entry| entry.starts_with("apply:"))
            .count();
        assert_eq!(apply_count, 2);
    }

    #[test]
    fn unresolved_crash_recovery_blocks_enabling_over_unknown_gamma() {
        let (controller, _trace) = controller(false);

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
    fn restore_failure_is_not_hidden_when_user_turns_feature_off() {
        let trace = Arc::new(Mutex::new(Trace::default()));
        let controller = NightVisionController::new(
            FakeFactory {
                trace,
                fail_apply: false,
                fail_restore: true,
                fail_recovery_cleanup: false,
            },
            std::env::temp_dir().join("unused-night-vision-recovery.json"),
            70,
        );
        controller.toggle_requested();
        controller.reconcile(Some(target("DISPLAY1", 101)));

        controller.toggle_requested();
        let state = controller.reconcile(None);

        assert!(state.gamma_applied);
        assert!(!state.applied);
        assert!(!state.supported);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.driver_rejected")
        );

        controller.toggle_requested();
        let retried = controller.reconcile(Some(target("DISPLAY1", 101)));
        assert!(retried.gamma_applied);
        assert!(!retried.applied);
        assert!(retried.requested);
        assert!(!retried.supported);
        let restore_attempts = controller
            .factory
            .trace
            .lock()
            .unwrap()
            .operations
            .iter()
            .filter(|operation| operation.starts_with("restore:"))
            .count();
        assert_eq!(restore_attempts, 2);
        let apply_attempts = controller
            .factory
            .trace
            .lock()
            .unwrap()
            .operations
            .iter()
            .filter(|operation| operation.starts_with("apply:"))
            .count();
        assert_eq!(
            apply_attempts, 1,
            "toggle-on must retry restore before any new gamma apply"
        );
    }

    #[test]
    fn cleanup_failure_never_claims_gamma_is_still_applied_or_blocks_reenable() {
        let trace = Arc::new(Mutex::new(Trace::default()));
        let controller = NightVisionController::new(
            FakeFactory {
                trace,
                fail_apply: false,
                fail_restore: false,
                fail_recovery_cleanup: true,
            },
            std::env::temp_dir().join("unused-night-vision-recovery.json"),
            70,
        );
        controller.toggle_requested();
        controller.reconcile(Some(target("DISPLAY1", 101)));

        controller.toggle_requested();
        let restored = controller.reconcile(None);
        assert!(!restored.applied, "display restore was already verified");

        controller.toggle_requested();
        let reapplied = controller.reconcile(Some(target("DISPLAY1", 101)));
        assert!(reapplied.requested && reapplied.gamma_applied && reapplied.supported);
        assert!(!reapplied.applied);
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
        let (x, y) = super::button_anchor(rect, 1.5, (164.0, 42.0), 12.0);

        assert_eq!((x, y), (1756, 218));
        assert!(x >= rect.0);
        assert!(y >= rect.1);
        assert!(x + (164.0_f64 * 1.5).round() as i32 <= rect.0 + rect.2);
        assert!(y + (42.0_f64 * 1.5).round() as i32 <= rect.1 + rect.3);
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
