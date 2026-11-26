//! Release-only raw worker. Python owns manifests, independent proof hashing and summaries.
use hoyadb_benchmark::{Harness, ResourceEvent, Result, object, rss_cycles, text};
use hoyadb_core::command::Json;
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    path::Path,
    time::Instant,
};
fn map(value: &Json) -> Result<&BTreeMap<String, Json>> {
    if let Json::Object(value) = value {
        Ok(value)
    } else {
        Err("worker object".into())
    }
}
fn string(value: &Json) -> Result<&str> {
    if let Json::String(value) = value {
        Ok(value)
    } else {
        Err("worker string".into())
    }
}
fn integer(value: &Json) -> Result<usize> {
    if let Json::Integer(value) = value {
        usize::try_from(*value).map_err(|_| "worker integer".into())
    } else {
        Err("worker integer".into())
    }
}
fn proof(harness: &mut Harness, phase: &str, out: &mut impl Write) -> Result<()> {
    writeln!(out, "PROOF_BEGIN {phase}").map_err(|_| "proof IO")?;
    harness.verify_to(|table, row| {
        let line = object([("table", text(table)), ("row", row.clone())])
            .canonical()
            .map_err(|_| "proof encoding")?;
        writeln!(out, "PROOF {line}").map_err(|_| "proof IO".into())
    })?;
    writeln!(out, "PROOF_END {phase}").map_err(|_| "proof IO")?;
    out.flush().map_err(|_| "proof flush".into())
}
fn resource_worker(
    directory: Option<&Path>,
    mut identity: BTreeMap<String, Json>,
    status: &str,
) -> Result<bool> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let protection = rss_cycles(directory, |event| {
        match event {
            ResourceEvent::Checkpoint(phase, cycle) => {
                let point = object([
                    ("phase", text(phase)),
                    (
                        "cycle",
                        cycle.map(|v| Json::Integer(v as i64)).unwrap_or(Json::Null),
                    ),
                ]);
                writeln!(
                    out,
                    "RSS {}",
                    point.canonical().map_err(|_| "RSS encoding")?
                )
                .map_err(|_| "RSS IO")?;
                out.flush().map_err(|_| "RSS flush")?;
                let mut acknowledgement = [0; 9];
                io::stdin()
                    .read_exact(&mut acknowledgement)
                    .map_err(|_| "RSS acknowledgement IO")?;
                if &acknowledgement != b"CONTINUE\n" {
                    return Err("RSS acknowledgement".into());
                }
            }
            ResourceEvent::ProofBegin(phase) => {
                writeln!(out, "PROOF_BEGIN {phase}").map_err(|_| "proof IO")?
            }
            ResourceEvent::ProofRow(table, row) => writeln!(
                out,
                "PROOF {}",
                object([("table", text(table)), ("row", row.clone())])
                    .canonical()
                    .map_err(|_| "proof encoding")?
            )
            .map_err(|_| "proof IO")?,
            ResourceEvent::ProofEnd(phase) => {
                writeln!(out, "PROOF_END {phase}").map_err(|_| "proof IO")?
            }
        }
        Ok(())
    })?;
    identity.insert("actual_protection".into(), protection);
    identity.insert("worker_arch".into(), text(std::env::consts::ARCH));
    identity.insert("worker_os".into(), text(std::env::consts::OS));
    let report = object([
        ("format", Json::Integer(1)),
        ("identity", Json::Object(identity)),
        ("kind", text("untimed-resources")),
        ("status", text(status)),
        ("cycles", Json::Integer(30)),
        ("runs", Json::Array(vec![])),
    ]);
    writeln!(
        out,
        "REPORT {}",
        report.canonical().map_err(|_| "resource report encoding")?
    )
    .map_err(|_| "resource report IO")?;
    out.flush().map_err(|_| "resource report flush")?;
    Ok(true)
}
fn worker() -> Result<bool> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2
        || ![
            "--config",
            "--cold-config",
            "--prepare-config",
            "--rss-config",
        ]
        .contains(&args[0].as_str())
    {
        return Err("requires worker mode and config FILE".into());
    }
    let cold = args[0] == "--cold-config";
    let resource = args[0] == "--rss-config";
    let prepare = args[0] == "--prepare-config";
    let config = Json::parse(&std::fs::read_to_string(&args[1]).map_err(|_| "worker config IO")?)
        .map_err(|_| "worker config JSON")?;
    let config = map(&config)?;
    if config.keys().map(String::as_str).collect::<Vec<_>>()
        != [
            "cohort",
            "directory",
            "engine",
            "identity",
            "run",
            "run_status",
        ]
    {
        return Err("worker config keys".into());
    }
    let workload =
        Json::parse(include_str!("../../workloads.json")).map_err(|_| "workload JSON")?;
    let workload = map(&workload)?;
    let Json::Array(cohorts) = &workload["cohorts"] else {
        return Err("workload cohorts".into());
    };
    let cohort = &config["cohort"];
    let parameters = map(cohort)?;
    let operation = string(&parameters["operation"])?;
    let n = integer(&parameters["N"])?;
    let engine = string(&config["engine"])?;
    if resource {
        if operation != "retained-rss"
            || n != 100
            || parameters.len() != 4
            || parameters.get("Q") != Some(&Json::Integer(0))
            || parameters.get("observers") != Some(&Json::Integer(1))
        {
            return Err("unknown frozen resource cohort".into());
        }
    } else if cold {
        let Json::Array(classes) = &workload["cold"] else {
            return Err("cold classes".into());
        };
        let declared=classes.iter().any(|class| {
            let Ok(class)=map(class) else {return false};
            let Some(Json::Array(engines))=class.get("engines") else {return false};
            let size=class.get("N");
            class.get("operation")==Some(&text(operation)) && engines.contains(&text(engine)) && (size==Some(&Json::Integer(n as i64)) || matches!(size,Some(Json::Array(sizes)) if sizes.contains(&Json::Integer(n as i64))))
        });
        if !declared
            || parameters.len() != 4
            || parameters.get("Q") != Some(&Json::Integer(0))
            || parameters.get("observers") != Some(&Json::Integer(0))
        {
            return Err("unknown frozen cold cohort".into());
        }
    } else if !cohorts.contains(cohort)
        || (prepare && (engine != "native-protected" || operation != "point"))
    {
        return Err("unknown frozen cohort".into());
    }
    let directory = match engine {
        "memory" if config["directory"] == Json::Null => None,
        "native-protected" => Some(Path::new(string(&config["directory"])?)),
        _ => return Err("worker engine".into()),
    };
    let mut identity = map(&config["identity"])?.clone();
    if identity.get("profile") != Some(&text("release"))
        || identity.get("unit")
            != Some(&text(if resource {
                "bytes"
            } else {
                "ns"
            }))
        || identity.get("class") != Some(&text(format!("rust-facade/{engine}")))
        || identity.get("parameters") != Some(cohort)
    {
        return Err("worker identity mismatch".into());
    }
    let run = integer(&config["run"])?;
    let sampling = map(&workload["sampling"])?;
    let runs = if cold {
        integer(&sampling["cold_processes"])?
    } else if prepare || resource {
        1
    } else {
        integer(&sampling["independent_runs"])?
    };
    if run == 0 || run > runs {
        return Err("worker run".into());
    }
    let run_status = string(&config["run_status"])?;
    if !["ok", "contaminated"].contains(&run_status) {
        return Err("worker status".into());
    }
    if resource {
        return resource_worker(directory, identity, run_status);
    }
    let mut harness = if cold {
        Harness::cold(n, directory, operation == "upgrade")?
    } else {
        Harness::new(n, directory)?
    };
    harness.set_queue(integer(&parameters["Q"])?)?;
    if operation == "reactivity" {
        harness.set_observers(
            integer(&parameters["observers"])?,
            string(&parameters["sharing"])?,
            string(&parameters["mode"])?,
        )?;
    }
    identity.insert("actual_protection".into(), harness.protection.clone());
    identity.insert("worker_arch".into(), text(std::env::consts::ARCH));
    identity.insert("worker_os".into(), text(std::env::consts::OS));
    let out = io::stdout();
    let mut out = io::BufWriter::new(out.lock());
    if cold {
        writeln!(out, "READY").map_err(|_| "ready IO")?;
        out.flush().map_err(|_| "ready flush")?;
    }
    proof(&mut harness, "initial", &mut out)?;
    let mut samples = Vec::new();
    let mut failed = false;
    let (warmups, sample_count) = if cold || prepare {
        (0, 1)
    } else {
        (
            integer(&sampling["warmups"])?,
            integer(&sampling["samples"])?,
        )
    };
    let mut minimum_tick = None;
    let mut previous = Instant::now();
    for _ in 0..10000 {
        let now = Instant::now();
        let tick = now.duration_since(previous).as_nanos();
        if tick > 0 {
            minimum_tick = Some(minimum_tick.map_or(tick, |old: u128| old.min(tick)));
        }
        previous = now;
    }

    for (phase, count) in [("warmup", warmups), ("measured", sample_count)] {
        if failed {
            break;
        }
        for index in 0..count {
            let attempted = Instant::now();
            let result = if cold || prepare {
                Ok(object([
                    (
                        "value",
                        Json::Integer(
                            i64::try_from(harness.open_ns).map_err(|_| "open duration overflow")?,
                        ),
                    ),
                    ("status", text("ok")),
                    ("correct", Json::Bool(true)),
                ]))
            } else {
                harness.sample(operation, index)
            };
            let mut sample = match result {
                Ok(Json::Object(value)) => value,
                Ok(_) => return Err("worker sample shape".into()),
                Err(error) => {
                    failed = true;
                    map(&object([
                        ("status", text("error")),
                        ("correct", Json::Bool(false)),
                        ("error", text(error)),
                        ("attempt_elapsed_ns", text(attempted.elapsed().as_nanos())),
                    ]))?
                    .clone()
                }
            };
            sample.insert("index".into(), Json::Integer(index as i64));
            sample.insert("phase".into(), text(phase));
            samples.push(Json::Object(sample));
            if failed {
                break;
            }
        }
    }
    if parameters["observers"] != Json::Integer(0) {
        harness.set_observers(0, "equivalent", "membership")?;
    }
    let final_proof = proof(&mut harness, "final", &mut out);
    if final_proof.is_err() {
        failed = true;
    }
    let report = object([
        ("format", Json::Integer(1)),
        ("identity", Json::Object(identity)),
        (
            "protocol",
            object([
                ("runs", Json::Integer(runs as i64)),
                ("warmups", Json::Integer(warmups as i64)),
                ("samples", Json::Integer(sample_count as i64)),
            ]),
        ),
        (
            "runs",
            Json::Array(vec![object([
                ("run", Json::Integer(run as i64)),
                (
                    "status",
                    text(if failed {
                        "failed"
                    } else {
                        run_status
                    }),
                ),
                ("samples", Json::Array(samples)),
                (
                    "metadata",
                    object([
                        ("pid", Json::Integer(std::process::id() as i64)),
                        ("open_setup_ns", text(harness.open_ns)),
                        ("proof_complete", Json::Bool(final_proof.is_ok())),
                        ("callback_poll_ms", Json::Integer(1)),
                        ("clock", text("std::time::Instant")),
                        ("hardware_resolution_ns", Json::Null),
                        (
                            "observed_min_positive_tick_ns",
                            minimum_tick.map(text).unwrap_or(Json::Null),
                        ),
                        (
                            "mode",
                            text(if cold {
                                "cold"
                            } else if prepare {
                                "setup"
                            } else {
                                "warm"
                            }),
                        ),
                    ]),
                ),
            ])]),
        ),
    ]);
    writeln!(
        out,
        "REPORT {}",
        report.canonical().map_err(|_| "report encoding")?
    )
    .map_err(|_| "report IO")?;
    out.flush().map_err(|_| "report flush")?;
    Ok(!failed)
}
fn main() {
    if cfg!(debug_assertions) {
        eprintln!("benchmark requires release profile");
        std::process::exit(2);
    }
    match worker() {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("benchmark failed: {error}");
            std::process::exit(1);
        }
    }
}
