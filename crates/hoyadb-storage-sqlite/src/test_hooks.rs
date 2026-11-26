//! SPEC-0037 R2.3: native, non-default fixture fault controls. A fixture installs
//! a trigger guarded by `hoya_test_fault('<name>')` while its store is closed,
//! then arms the named fault for that store's path while its owner stays live.
//! Nothing here exists in a normal build.
use rusqlite::{Connection, functions::FunctionFlags};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

static ARMED: Mutex<BTreeSet<(PathBuf, String)>> = Mutex::new(BTreeSet::new());

fn armed() -> MutexGuard<'static, BTreeSet<(PathBuf, String)>> {
    // A panicking fixture must not leave other fixtures unable to disarm.
    ARMED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One armed fault, scoped to the exact path its store was opened with.
/// Dropping the guard disarms it, including during a panic.
#[must_use = "the fault is disarmed when this guard drops"]
pub struct ArmedFault(PathBuf, String);

pub fn arm(path: impl AsRef<Path>, name: &str) -> ArmedFault {
    let key = (path.as_ref().to_path_buf(), name.to_owned());
    assert!(armed().insert(key.clone()), "fault {name} is already armed");
    ArmedFault(key.0, key.1)
}

impl Drop for ArmedFault {
    fn drop(&mut self) {
        armed().remove(&(std::mem::take(&mut self.0), std::mem::take(&mut self.1)));
    }
}

pub(crate) fn register(connection: &Connection, path: &Path) -> rusqlite::Result<()> {
    let path = path.to_path_buf();
    connection.create_scalar_function(
        "hoya_test_fault",
        1,
        FunctionFlags::SQLITE_UTF8,
        move |context| {
            let name = context.get::<String>(0)?;
            Ok(armed().contains(&(path.clone(), name)))
        },
    )
}
