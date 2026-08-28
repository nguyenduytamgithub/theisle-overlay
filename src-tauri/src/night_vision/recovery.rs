use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

use super::GammaRamp;

const RECOVERY_VERSION: u8 = 1;
const RAMP_ENTRY_COUNT: usize = 3 * 256;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct RecoveryRecord {
    version: u8,
    pub(crate) display_name: String,
    ramp: Vec<u16>,
}

impl RecoveryRecord {
    pub(crate) fn from_ramp(display_name: impl Into<String>, ramp: &GammaRamp) -> Self {
        Self {
            version: RECOVERY_VERSION,
            display_name: display_name.into(),
            ramp: ramp
                .iter()
                .flat_map(|channel| channel.iter().copied())
                .collect(),
        }
    }

    pub(crate) fn to_ramp(&self) -> Result<GammaRamp, RecoveryError> {
        if self.version != RECOVERY_VERSION {
            return Err(RecoveryError::Invalid(format!(
                "unsupported recovery version {}",
                self.version
            )));
        }
        if self.ramp.len() != RAMP_ENTRY_COUNT {
            return Err(RecoveryError::Invalid(format!(
                "recovery ramp must contain exactly {RAMP_ENTRY_COUNT} entries, got {}",
                self.ramp.len()
            )));
        }

        let mut ramp = [[0u16; 256]; 3];
        for (channel, values) in ramp.iter_mut().zip(self.ramp.chunks_exact(256)) {
            channel.copy_from_slice(values);
        }
        Ok(ramp)
    }
}

#[derive(Debug)]
pub(crate) enum RecoveryError {
    Io(io::Error),
    Json(serde_json::Error),
    Windows(windows::core::Error),
    Invalid(String),
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "recovery I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "recovery JSON failed: {error}"),
            Self::Windows(error) => write!(formatter, "recovery replace failed: {error}"),
            Self::Invalid(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for RecoveryError {}

impl From<io::Error> for RecoveryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for RecoveryError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<windows::core::Error> for RecoveryError {
    fn from(error: windows::core::Error) -> Self {
        Self::Windows(error)
    }
}

pub(crate) fn read_validated(path: &Path) -> Result<RecoveryRecord, RecoveryError> {
    let record: RecoveryRecord = serde_json::from_reader(BufReader::new(File::open(path)?))?;
    record.to_ramp()?;
    Ok(record)
}

pub(crate) fn write_atomic(path: &Path, record: &RecoveryRecord) -> Result<(), RecoveryError> {
    let parent = path.parent().ok_or_else(|| {
        RecoveryError::Invalid("recovery path has no parent directory".to_string())
    })?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    {
        let file = File::create(&temporary)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, record)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
    }

    let temporary_wide = wide_path(&temporary);
    let path_wide = wide_path(path);
    unsafe {
        MoveFileExW(
            PCWSTR(temporary_wide.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )?;
    }
    Ok(())
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{read_validated, write_atomic, RecoveryRecord};
    use crate::night_vision::GammaRamp;
    use std::path::PathBuf;

    fn fixture_ramp() -> GammaRamp {
        std::array::from_fn(|channel| {
            std::array::from_fn(|index| ((channel * 256 + index) * 80) as u16)
        })
    }

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "theisle-night-vision-{}-{name}",
                uuid::Uuid::new_v4()
            ))
            .join("night-vision-recovery.json")
    }

    #[test]
    fn recovery_round_trip_preserves_display_and_every_ramp_entry() {
        let path = test_path("round-trip");
        let expected = RecoveryRecord::from_ramp(r"\\.\MÀNHÌNH-1", &fixture_ramp());

        write_atomic(&path, &expected).unwrap();
        let actual = read_validated(&path).unwrap();

        assert_eq!(actual.display_name, expected.display_name);
        assert_eq!(actual.to_ramp().unwrap(), fixture_ramp());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn malformed_ramp_lengths_are_rejected_before_restore() {
        for bad_len in [767, 769] {
            let path = test_path(&format!("bad-{bad_len}"));
            let parent = path.parent().unwrap();
            std::fs::create_dir_all(parent).unwrap();
            let json = serde_json::json!({
                "version": 1,
                "display_name": r"\\.\DISPLAY1",
                "ramp": vec![0u16; bad_len],
            });
            std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();

            let error = read_validated(&path).unwrap_err();
            assert!(error.to_string().contains("768"));
            std::fs::remove_dir_all(parent).unwrap();
        }
    }

    #[test]
    fn atomic_write_replaces_a_complete_previous_record() {
        let path = test_path("replace");
        let first = RecoveryRecord::from_ramp(r"\\.\DISPLAY1", &fixture_ramp());
        let mut second_ramp = fixture_ramp();
        second_ramp[2][255] = u16::MAX;
        let second = RecoveryRecord::from_ramp(r"\\.\DISPLAY2", &second_ramp);

        write_atomic(&path, &first).unwrap();
        write_atomic(&path, &second).unwrap();

        let actual = read_validated(&path).unwrap();
        assert_eq!(actual.display_name, r"\\.\DISPLAY2");
        assert_eq!(actual.to_ramp().unwrap(), second_ramp);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
