//! Bounded warm-cohort cost diagnostic: stage wall times only, never latency samples.
//! Uses the same library harness as the measured worker; the parent bounds its runtime.
use hoyadb_benchmark::{Harness, Result, object, text};
use hoyadb_core::command::Json;
use std::{
    io::{self, Write},
    path::Path,
    time::Instant,
};
fn emit(
    out: &mut impl Write,
    stage: &str,
    index: Option<usize>,
    started: Instant,
    value: Option<&Json>,
) -> Result<()> {
    let wall = i64::try_from(started.elapsed().as_nanos()).map_err(|_| "wall overflow")?;
    let line = object([
        ("stage", text(stage)),
        ("wall_ns", Json::Integer(wall)),
        (
            "index",
            index.map_or(Json::Null, |v| Json::Integer(v as i64)),
        ),
        ("timed_value_ns", value.cloned().unwrap_or(Json::Null)),
    ])
    .canonical()
    .map_err(|_| "diagnostic encoding")?;
    writeln!(out, "DIAGNOSTIC {line}").map_err(|_| "diagnostic IO")?;
    out.flush().map_err(|_| "diagnostic flush".into())
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, operation, n, samples, directory] = args.as_slice() else {
        return Err("usage: cost-diagnostic <operation> <N> <samples> <directory|->".into());
    };
    let n: usize = n.parse().map_err(|_| "N")?;
    let samples: usize = samples.parse().map_err(|_| "samples")?;
    if !(2..=5).contains(&samples) || operation != "reset" {
        return Err("diagnostic supports reset with 2..=5 samples".into());
    }
    let directory = (directory != "-").then(|| Path::new(directory.as_str()));
    let mut out = io::stdout().lock();
    let started = Instant::now();
    let mut harness = Harness::new(n, directory)?;
    harness.set_queue(0)?;
    emit(&mut out, "setup", None, started, None)?;
    let started = Instant::now();
    harness.verify()?;
    emit(&mut out, "fixture_validation", None, started, None)?;
    for index in 0..samples {
        let started = Instant::now();
        let sample = harness.sample(operation, index)?;
        let Json::Object(fields) = &sample else {
            return Err("sample shape".into());
        };
        emit(
            &mut out,
            "sample",
            Some(index),
            started,
            fields.get("value"),
        )?;
    }
    Ok(())
}
fn main() {
    if cfg!(debug_assertions) {
        eprintln!("diagnostic requires release profile");
        std::process::exit(2);
    }
    if let Err(error) = run() {
        eprintln!("diagnostic failed: {error}");
        std::process::exit(1);
    }
}
