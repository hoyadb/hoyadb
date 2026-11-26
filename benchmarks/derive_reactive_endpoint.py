#!/usr/bin/env python3
"""DEF-0020 versioned endpoint derivation for complete accepted native reactive originals.

Only the endpoint identity of operation=reactivity reports changes. Originals are
read-only inputs captured once; every sample is retained unchanged and provenance
records the original bytes and this transform's source digest. Failed, interrupted,
contaminated or inconsistent captures reject before any output is created.
"""
import argparse
import copy
import hashlib
import os
from pathlib import Path
import re
import sys
from run import RunnerError, canonical, proof_records, require, strict_json, write_new
from summary import InvalidSamples, summarize

TRANSFORM="def-0020-reactive-endpoint/1"
ORIGINAL="serialize/facade-call/decoded-return"
DERIVED="caller-harness-invocation-to-last-callback"
CALLBACK=("dispatch_complete_ns","first_callback_ns","last_callback_ns",
          "return_to_first_callback_ns","return_to_last_callback_ns","callback_count")
PROTOCOL={"runs":5,"samples":30,"warmups":5}
RAW=re.compile(r"warm-(memory|native-protected)-([0-9]{2})-([1-5])-raw\.json")
HEX=re.compile(r"[a-f0-9]{64}")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def capture(root):
    files={}
    for path in sorted(root.rglob("*")):
        require(not path.is_symlink(),"symlink in original experiment")
        if path.is_file():
            files[path.relative_to(root).as_posix()]=path.read_bytes()
    return files


def oracle(n):
    rows=[(canonical(row)+"\n").encode() for row in proof_records(n)]
    return {"count":len(rows),"sha256":digest(b"".join(rows))}


def counter(value):
    return type(value) is int and value>=0


def checked_group(files,manifest,label,runs):
    require(set(runs)=={1,2,3,4,5},"incomplete independent runs")
    reports=[]
    for run in range(1,6):
        config=strict_json(files.get(f"{label}-{run}-config.json",b"null"))
        raw=strict_json(runs[run])
        require(isinstance(config,dict) and isinstance(raw,dict),"missing config/raw")
        require(raw.get("status")=="ok" and raw.get("exit_code")==0 and "failure" not in raw,"failed raw worker")
        report=raw.get("report")
        require(isinstance(report,dict) and report.get("format")==1 and report.get("protocol")==PROTOCOL,"invalid original report")
        identity=report.get("identity")
        require(isinstance(identity,dict) and identity.get("endpoint")==ORIGINAL,"not an original generic-label report")
        require(identity.get("artifact_sha256")==manifest["artifact"]["sha256"] and identity.get("source_sha256")==manifest["source_before"]["sha256"]
                and identity.get("workload_sha256")==manifest["workload_sha256"],"mixed artifact/source/workload identity")
        parameters=identity.get("parameters")
        require(isinstance(parameters,dict) and parameters.get("operation")=="reactivity" and counter(parameters.get("observers")),"not a reactive cohort")
        require(config.get("run")==run and config.get("run_status")=="ok" and config.get("cohort")==parameters
                and isinstance(config.get("identity"),dict) and all(identity.get(key)==value for key,value in config["identity"].items()),"configuration/report mismatch")
        expected=oracle(parameters.get("N"))
        require(raw.get("proof")=={"initial":expected,"final":expected},"independent fixture proof mismatch")
        require(isinstance(report.get("runs"),list) and len(report["runs"])==1,"invalid per-run document")
        result=report["runs"][0]
        require(result.get("run")==run and result.get("status")=="ok","failed or contaminated run")
        samples=result.get("samples")
        require(isinstance(samples,list) and [(item.get("phase"),item.get("index")) for item in samples if isinstance(item,dict)]
                ==[("warmup",index) for index in range(5)]+[("measured",index) for index in range(30)],"incomplete sample sequence")
        for item in samples:
            require(item.get("status")=="ok" and item.get("correct") is True,"failed sample")
            require(counter(item.get("value")) and all(counter(item.get(key)) for key in CALLBACK),"invalid callback capture")
            require(item["value"]==item["last_callback_ns"],"primary value is not the last callback clock")
            require(item["first_callback_ns"]<=item["last_callback_ns"] and item["return_to_first_callback_ns"]<=item["return_to_last_callback_ns"],"inconsistent callback ordering")
            require(item["callback_count"]==parameters["observers"],"callback count mismatch")
        reports.append(report)
    require(all(report["identity"]==reports[0]["identity"] for report in reports),"mixed run identity")
    accepted=strict_json(files.get(f"{label}-summary.json",b"null"))
    try:
        require(accepted==summarize(reports),"original accepted summary mismatch")
        derived=copy.deepcopy(reports)
        for report in derived:
            report["identity"]["endpoint"]=DERIVED
        summary=summarize(derived)
    except InvalidSamples as error:
        raise RunnerError("invalid original samples") from error
    relabelled=copy.deepcopy(accepted);relabelled["identity"]["endpoint"]=DERIVED
    require(summary==relabelled,"derived statistics differ from original")
    return derived,summary


def derive(source,output):
    output=Path(output)
    require(not os.path.lexists(output),"derivation output must be new")
    source=Path(source).resolve(strict=True)
    files=capture(source)
    originals={name:digest(data) for name,data in files.items()}
    manifest=strict_json(files.get("manifest.json",b"null"))
    require(isinstance(manifest,dict) and manifest.get("format")==1 and manifest.get("status")=="ok" and manifest.get("run_status")=="ok"
            and manifest.get("quiet_window_attested_by_caller") is True and manifest.get("build_exit_code")==0 and "failure" not in manifest,
            "original experiment is not complete and accepted")
    before=manifest.get("source_before")
    require(isinstance(before,dict) and isinstance(before.get("sha256"),str) and HEX.fullmatch(before["sha256"]) and manifest.get("source_after")==before,"source identity changed")
    require(isinstance(manifest.get("artifact"),dict) and isinstance(manifest["artifact"].get("sha256"),str) and HEX.fullmatch(manifest["artifact"]["sha256"])
            and isinstance(manifest.get("workload_sha256"),str) and HEX.fullmatch(manifest["workload_sha256"]),"invalid artifact/workload identity")
    groups={}
    for name,data in files.items():
        match=RAW.fullmatch(name)
        if match:
            groups.setdefault(f"warm-{match[1]}-{match[2]}",{})[int(match[3])]=data
    derived,skipped={},[]
    for label in sorted(groups):
        config=strict_json(files.get(f"{label}-1-config.json",b"null"))
        if isinstance(config,dict) and isinstance(config.get("cohort"),dict) and config["cohort"].get("operation")!="reactivity":
            skipped.append(label)
            continue
        derived[label]=checked_group(files,manifest,label,groups[label])
    require(derived,"no complete reactive cohort to derive")
    output.mkdir(parents=True,exist_ok=False)
    written={}
    for label,(reports,summary) in derived.items():
        for run,report in enumerate(reports,1):
            written[f"derived-{label}-{run}-report.json"]=report
        written[f"derived-{label}-summary.json"]=summary
    for name,value in written.items():
        write_new(output/name,value)
    require({name:digest(data) for name,data in capture(source).items()}==originals,"original experiment changed during derivation")
    provenance={"format":1,"transform":TRANSFORM,"transform_source_sha256":digest(Path(__file__).read_bytes()),
                "from_endpoint":ORIGINAL,"to_endpoint":DERIVED,"source_directory":str(source),"originals":originals,
                "derived_groups":sorted(derived),"not_derived_groups":skipped,
                "derived":{name:digest((canonical(value)+"\n").encode()) for name,value in sorted(written.items())},
                "qualification":"endpoint identity relabelled only; samples, values and proofs are the unchanged originals; not pooled with directly labelled observations"}
    write_new(output/"provenance.json",provenance)
    return provenance


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source",required=True,type=Path)
    parser.add_argument("--output",required=True,type=Path)
    args=parser.parse_args()
    try:
        provenance=derive(args.source,args.output)
    except (RunnerError,OSError) as error:
        print("derivation rejected: "+str(error),file=sys.stderr)
        return 1
    print(canonical({"derived_groups":provenance["derived_groups"],"not_derived_groups":provenance["not_derived_groups"]}))
    return 0


if __name__=="__main__":
    raise SystemExit(main())
