use std::ffi::c_void;
use std::fmt;
use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetMonitorInfoW, MonitorFromWindow, HDC, MONITORINFOEXW,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};

use super::curve::{ramps_match, READBACK_TOLERANCE};
use super::recovery::{read_validated, write_atomic, RecoveryError, RecoveryRecord};
use super::GammaRamp;

#[derive(Debug)]
pub(crate) enum NightVisionError {
    Driver(String),
    Recovery(RecoveryError),
    ReadbackRejected,
    RestoreRejected,
}

impl fmt::Display for NightVisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => formatter.write_str(error),
            Self::Recovery(error) => write!(formatter, "{error}"),
            Self::ReadbackRejected => {
                formatter.write_str("driver rejected the requested gamma ramp")
            }
            Self::RestoreRejected => {
                formatter.write_str("driver did not restore the original gamma ramp")
            }
        }
    }
}

impl std::error::Error for NightVisionError {}

impl From<RecoveryError> for NightVisionError {
    fn from(error: RecoveryError) -> Self {
        Self::Recovery(error)
    }
}

pub(crate) trait GammaApi: Send {
    fn read(&mut self, display_name: &str) -> Result<GammaRamp, NightVisionError>;
    fn write(&mut self, display_name: &str, ramp: &GammaRamp) -> Result<(), NightVisionError>;
}

pub(crate) struct DisplayGamma<A: GammaApi = Win32GammaApi> {
    api: A,
    display_name: String,
    original: GammaRamp,
    recovery_path: PathBuf,
}

impl<A: GammaApi> DisplayGamma<A> {
    pub(crate) fn from_snapshot(
        api: A,
        display_name: String,
        original: GammaRamp,
        recovery_path: PathBuf,
    ) -> Self {
        Self {
            api,
            display_name,
            original,
            recovery_path,
        }
    }

    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }

    pub(crate) fn original(&self) -> &GammaRamp {
        &self.original
    }

    pub(crate) fn read_current(&mut self) -> Result<GammaRamp, NightVisionError> {
        self.api.read(&self.display_name)
    }

    pub(crate) fn apply_verified(&mut self, requested: &GammaRamp) -> Result<(), NightVisionError> {
        write_atomic(
            &self.recovery_path,
            &RecoveryRecord::from_ramp(&self.display_name, &self.original),
        )?;

        if let Err(error) = self.api.write(&self.display_name, requested) {
            let _ = self.restore();
            return Err(error);
        }
        let actual = self.api.read(&self.display_name)?;
        if ramps_match(requested, &actual, READBACK_TOLERANCE) {
            return Ok(());
        }

        match self.restore() {
            Ok(()) => Err(NightVisionError::ReadbackRejected),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn restore(&mut self) -> Result<(), NightVisionError> {
        self.api.write(&self.display_name, &self.original)?;
        let actual = self.api.read(&self.display_name)?;
        if !ramps_match(&self.original, &actual, READBACK_TOLERANCE) {
            return Err(NightVisionError::RestoreRejected);
        }
        match std::fs::remove_file(&self.recovery_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(NightVisionError::Driver(format!(
                "restored gamma but could not remove recovery record: {error}"
            ))),
        }
    }
}

impl<A: GammaApi> Drop for DisplayGamma<A> {
    fn drop(&mut self) {
        if self.recovery_path.exists() {
            if let Err(error) = self.restore() {
                log::error!("night vision drop restore failed: {error}");
            }
        }
    }
}

impl DisplayGamma<Win32GammaApi> {
    pub(crate) fn for_game_window(
        hwnd: isize,
        recovery_path: PathBuf,
    ) -> Result<Self, NightVisionError> {
        let display_name = display_name_for_window(hwnd)?;
        let mut api = Win32GammaApi;
        let original = api.read(&display_name)?;
        Ok(Self::from_snapshot(
            api,
            display_name,
            original,
            recovery_path,
        ))
    }
}

pub(crate) fn restore_recovery_record(path: &Path) -> Result<bool, NightVisionError> {
    if !path.exists() {
        return Ok(false);
    }
    let record = read_validated(path)?;
    let ramp = record.to_ramp()?;
    let mut api = Win32GammaApi;
    api.write(&record.display_name, &ramp)?;
    let actual = api.read(&record.display_name)?;
    if !ramps_match(&ramp, &actual, READBACK_TOLERANCE) {
        return Err(NightVisionError::RestoreRejected);
    }
    std::fs::remove_file(path).map_err(|error| {
        NightVisionError::Driver(format!(
            "recovery succeeded but record removal failed: {error}"
        ))
    })?;
    Ok(true)
}

pub(crate) struct Win32GammaApi;

impl GammaApi for Win32GammaApi {
    fn read(&mut self, display_name: &str) -> Result<GammaRamp, NightVisionError> {
        let dc = DisplayDc::open(display_name)?;
        let mut ramp = [[0u16; 256]; 3];
        let succeeded =
            unsafe { GetDeviceGammaRamp(dc.0, ramp.as_mut_ptr().cast::<c_void>()).as_bool() };
        if !succeeded {
            return Err(last_driver_error("GetDeviceGammaRamp"));
        }
        Ok(ramp)
    }

    fn write(&mut self, display_name: &str, ramp: &GammaRamp) -> Result<(), NightVisionError> {
        let dc = DisplayDc::open(display_name)?;
        let succeeded =
            unsafe { SetDeviceGammaRamp(dc.0, ramp.as_ptr().cast::<c_void>()).as_bool() };
        if !succeeded {
            return Err(last_driver_error("SetDeviceGammaRamp"));
        }
        Ok(())
    }
}

struct DisplayDc(HDC);

impl DisplayDc {
    fn open(display_name: &str) -> Result<Self, NightVisionError> {
        let wide = wide(display_name);
        let dc = unsafe { CreateDCW(PCWSTR::null(), PCWSTR(wide.as_ptr()), PCWSTR::null(), None) };
        if dc.is_invalid() {
            return Err(last_driver_error("CreateDCW"));
        }
        Ok(Self(dc))
    }
}

impl Drop for DisplayDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

pub(crate) fn display_name_for_window(hwnd: isize) -> Result<String, NightVisionError> {
    let monitor = unsafe { MonitorFromWindow(HWND(hwnd as *mut c_void), MONITOR_DEFAULTTONEAREST) };
    if monitor.is_invalid() {
        return Err(last_driver_error("MonitorFromWindow"));
    }

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let succeeded = unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo).as_bool() };
    if !succeeded {
        return Err(last_driver_error("GetMonitorInfoW"));
    }
    let end = info
        .szDevice
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(info.szDevice.len());
    String::from_utf16(&info.szDevice[..end])
        .map_err(|error| NightVisionError::Driver(format!("invalid display name: {error}")))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_driver_error(operation: &str) -> NightVisionError {
    NightVisionError::Driver(format!(
        "{operation} failed: {}",
        windows::core::Error::from_thread()
    ))
}

#[cfg(test)]
mod tests {
    use super::{DisplayGamma, GammaApi, NightVisionError};
    use crate::night_vision::curve::lifted_ramp;
    use crate::night_vision::GammaRamp;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct FakeTrace {
        writes: Vec<GammaRamp>,
        recovery_seen_before_first_write: bool,
    }

    struct FakeGammaApi {
        reads: VecDeque<GammaRamp>,
        recovery_path: PathBuf,
        fail_writes_after: Option<usize>,
        trace: Arc<Mutex<FakeTrace>>,
    }

    impl FakeGammaApi {
        fn new(recovery_path: PathBuf, reads: Vec<GammaRamp>) -> (Self, Arc<Mutex<FakeTrace>>) {
            let trace = Arc::new(Mutex::new(FakeTrace::default()));
            (
                Self {
                    reads: reads.into(),
                    recovery_path,
                    fail_writes_after: None,
                    trace: trace.clone(),
                },
                trace,
            )
        }
    }

    impl GammaApi for FakeGammaApi {
        fn read(&mut self, _display_name: &str) -> Result<GammaRamp, NightVisionError> {
            self.reads
                .pop_front()
                .ok_or_else(|| NightVisionError::Driver("fake read queue exhausted".to_string()))
        }

        fn write(&mut self, _display_name: &str, ramp: &GammaRamp) -> Result<(), NightVisionError> {
            let mut trace = self.trace.lock().unwrap();
            if trace.writes.is_empty() {
                trace.recovery_seen_before_first_write = self.recovery_path.exists();
            }
            if self
                .fail_writes_after
                .is_some_and(|limit| trace.writes.len() >= limit)
            {
                return Err(NightVisionError::Driver("fake write failed".to_string()));
            }
            trace.writes.push(*ramp);
            Ok(())
        }
    }

    fn recovery_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("theisle-gamma-api-{}-{name}", uuid::Uuid::new_v4()))
            .join("night-vision-recovery.json")
    }

    fn cleanup(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::remove_dir_all(parent).unwrap();
        }
    }

    #[test]
    fn verified_apply_persists_original_before_touching_the_driver() {
        let path = recovery_path("apply");
        let original = lifted_ramp(0);
        let requested = lifted_ramp(70);
        let (api, trace) = FakeGammaApi::new(path.clone(), vec![requested]);
        let mut display =
            DisplayGamma::from_snapshot(api, r"\\.\DISPLAY1".to_string(), original, path.clone());

        display.apply_verified(&requested).unwrap();

        let trace = trace.lock().unwrap();
        assert!(trace.recovery_seen_before_first_write);
        assert_eq!(trace.writes, vec![requested]);
        assert!(
            path.exists(),
            "recovery must remain while gamma is modified"
        );
        cleanup(&path);
    }

    #[test]
    fn rejected_readback_restores_original_and_removes_recovery_after_verification() {
        let path = recovery_path("reject");
        let original = lifted_ramp(0);
        let requested = lifted_ramp(70);
        let rejected = lifted_ramp(25);
        let (api, trace) = FakeGammaApi::new(path.clone(), vec![rejected, original]);
        let mut display =
            DisplayGamma::from_snapshot(api, r"\\.\DISPLAY1".to_string(), original, path.clone());

        let error = display.apply_verified(&requested).unwrap_err();

        assert!(matches!(error, NightVisionError::ReadbackRejected));
        assert_eq!(trace.lock().unwrap().writes, vec![requested, original]);
        assert!(!path.exists(), "verified restore should clear recovery");
        cleanup(&path);
    }

    #[test]
    fn failed_restore_keeps_recovery_for_the_next_launch() {
        let path = recovery_path("restore-failure");
        let original = lifted_ramp(0);
        let requested = lifted_ramp(70);
        let rejected = lifted_ramp(25);
        let (mut api, _trace) = FakeGammaApi::new(path.clone(), vec![rejected]);
        api.fail_writes_after = Some(1);
        let mut display =
            DisplayGamma::from_snapshot(api, r"\\.\DISPLAY1".to_string(), original, path.clone());

        assert!(display.apply_verified(&requested).is_err());
        assert!(
            path.exists(),
            "failed restore must preserve recovery evidence"
        );
        cleanup(&path);
    }

    #[test]
    fn dropping_an_applied_display_restores_the_original_ramp() {
        let path = recovery_path("drop-restore");
        let original = lifted_ramp(0);
        let requested = lifted_ramp(70);
        let (api, trace) = FakeGammaApi::new(path.clone(), vec![requested, original]);
        let mut display =
            DisplayGamma::from_snapshot(api, r"\\.\DISPLAY1".to_string(), original, path.clone());
        display.apply_verified(&requested).unwrap();

        drop(display);

        assert_eq!(trace.lock().unwrap().writes, vec![requested, original]);
        assert!(!path.exists());
        cleanup(&path);
    }
}
