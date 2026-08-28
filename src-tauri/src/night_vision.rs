mod curve;
mod recovery;
mod windows;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, State};

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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NightVisionState {
    pub requested: bool,
    pub applied: bool,
    pub supported: bool,
    pub strength: u8,
    pub error_key: Option<String>,
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
    recovery_blocked: bool,
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
                },
                session: None,
                applied_strength: None,
                recovery_blocked: false,
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
        inner.state.error_key = None;
        if inner.state.requested {
            inner.state.supported = true;
        }
        inner.state.clone()
    }

    pub(crate) fn block_for_recovery_error(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.recovery_blocked = true;
        inner.state.requested = false;
        inner.state.applied = false;
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
        }
        inner.state.clone()
    }

    pub(crate) fn reconcile(&self, game: Option<GameTarget>) -> NightVisionState {
        let mut inner = self.inner.lock_safe();

        if inner.recovery_blocked {
            return inner.state.clone();
        }

        if !inner.state.requested {
            restore_session(&mut inner);
            inner.state.applied = false;
            if inner.state.supported {
                inner.state.error_key = None;
            }
            return inner.state.clone();
        }

        let Some(target) = game else {
            restore_session(&mut inner);
            inner.state.applied = false;
            if inner.state.supported {
                inner.state.error_key = Some("night_vision.waiting_for_game".to_string());
            }
            return inner.state.clone();
        };

        if !inner.state.supported {
            return inner.state.clone();
        }

        let changed_display = inner
            .session
            .as_ref()
            .is_some_and(|session| session.display_name() != target.display_name);
        if changed_display {
            restore_session(&mut inner);
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
        if !inner.state.applied || inner.applied_strength != Some(strength) {
            let ramp = curve::lifted_ramp(strength);
            let result = inner
                .session
                .as_mut()
                .expect("session exists after factory open")
                .apply(&ramp);
            match result {
                Ok(()) => {
                    inner.state.applied = true;
                    inner.state.supported = true;
                    inner.state.error_key = None;
                    inner.applied_strength = Some(strength);
                }
                Err(error) => {
                    restore_session(&mut inner);
                    mark_failed(&mut inner, &error);
                }
            }
        }

        inner.state.clone()
    }

    pub(crate) fn restore_for_exit(&self) -> NightVisionState {
        let mut inner = self.inner.lock_safe();
        inner.state.requested = false;
        restore_session(&mut inner);
        inner.state.applied = false;
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
        Err(error) => {
            log::error!("night vision crash recovery failed; feature blocked: {error}");
            let state = night_vision.controller.block_for_recovery_error();
            emit_state(app, &state);
        }
    }

    spawn_supervisor(app.clone());
}

pub fn restore_before_exit(app: &AppHandle) {
    let night_vision = app.state::<NightVision>();
    let state = night_vision.controller.restore_for_exit();
    if state.error_key.is_some() {
        log::error!("night vision: gamma restore on exit was not verified");
    }
    emit_state(app, &state);
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

fn restore_session<S: GammaSession>(inner: &mut ControllerInner<S>) {
    if let Some(session) = inner.session.as_mut() {
        if let Err(error) = session.restore() {
            inner.state.supported = false;
            inner.state.error_key = Some(error_key(&error).to_string());
        }
    }
    inner.session = None;
    inner.state.applied = false;
    inner.applied_strength = None;
}

fn mark_failed<S: GammaSession>(inner: &mut ControllerInner<S>, error: &NightVisionError) {
    inner.state.applied = false;
    inner.state.supported = false;
    inner.state.error_key = Some(error_key(error).to_string());
    inner.applied_strength = None;
}

fn error_key(error: &NightVisionError) -> &'static str {
    match error {
        NightVisionError::Recovery(_) => "night_vision.recovery_error",
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
    }

    struct FakeSession {
        display_name: String,
        trace: Arc<Mutex<Trace>>,
        fail_apply: bool,
        fail_restore: bool,
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
    fn startup_is_off_and_active_game_toggle_applies_strength_70() {
        let (controller, trace) = controller(false);
        assert!(!controller.state().requested);
        assert!(!controller.state().applied);
        assert!(controller.state().supported);

        controller.toggle_requested();
        let state = controller.reconcile(Some(target("DISPLAY1", 101)));

        assert!(state.requested);
        assert!(state.applied);
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
        assert!(back.requested && back.applied);
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
        assert!(controller.state().applied);
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
            },
            std::env::temp_dir().join("unused-night-vision-recovery.json"),
            70,
        );
        controller.toggle_requested();
        controller.reconcile(Some(target("DISPLAY1", 101)));

        controller.toggle_requested();
        let state = controller.reconcile(None);

        assert!(!state.applied);
        assert!(!state.supported);
        assert_eq!(
            state.error_key.as_deref(),
            Some("night_vision.driver_rejected")
        );
    }
}
