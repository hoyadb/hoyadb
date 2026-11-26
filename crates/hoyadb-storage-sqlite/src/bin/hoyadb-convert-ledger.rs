//! Explicit offline operator tool; never opens an application-owned fixture in tests.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use hoyadb_storage_sqlite::convert::{
        ConversionMode, ConvertError, ConvertOptions, SourceProtection, run,
    };
    use std::{ffi::OsStr, path::PathBuf};

    fn parse() -> Result<ConvertOptions, ConvertError> {
        let mut args = std::env::args_os().skip(1);
        let (mut store, mut to, mut protection, mut mode) = (None, None, None, None);
        let invalid = || ConvertError::InvalidSource;
        while let Some(flag) = args.next() {
            match flag.to_str() {
                Some("--store") if store.is_none() => {
                    store = Some(PathBuf::from(args.next().ok_or_else(invalid)?));
                }
                Some("--to") if to.is_none() => {
                    to = Some(match args.next().as_deref().and_then(OsStr::to_str) {
                        Some("v2") => 2,
                        Some("v3") => 3,
                        _ => return Err(invalid()),
                    });
                }
                Some("--key-file") if protection.is_none() => {
                    protection = Some(SourceProtection::KeyFile(PathBuf::from(
                        args.next().ok_or_else(invalid)?,
                    )));
                }
                Some("--unprotected") if protection.is_none() => {
                    protection = Some(SourceProtection::Unprotected);
                }
                Some("--dry-run") if mode.is_none() => mode = Some(ConversionMode::DryRun),
                Some("--backup") if mode.is_none() => {
                    mode = Some(ConversionMode::Backup(PathBuf::from(
                        args.next().ok_or_else(invalid)?,
                    )));
                }
                _ => return Err(invalid()),
            }
        }
        Ok(ConvertOptions {
            store: store.ok_or_else(invalid)?,
            to: to.ok_or_else(invalid)?,
            protection: protection.ok_or_else(invalid)?,
            mode: mode.ok_or_else(invalid)?,
        })
    }

    match parse().and_then(run) {
        Ok(report) => {
            println!(
                "{{\"from\":{},\"to\":{},\"metadata_entries\":{},\"packets_rewritten\":{}}}",
                report.from, report.to, report.metadata_entries, report.packets_rewritten
            );
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(error.exit_code())
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    compile_error!("the ledger converter requires a native target");
}
