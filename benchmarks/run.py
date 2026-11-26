#!/usr/bin/env python3
"""TC-0037 native/memory orchestration; raw proof/statistics remain independent."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import signal
import platform
import shutil
import tomllib
import sys
import tempfile
import time


class RunnerError(ValueError):
    pass


def require(condition,message):
    if not condition:
        raise RunnerError(message)


def canonical(value):
    return json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(",",":"),allow_nan=False)


def tag(kind,value):
    return {"kind":kind,"value":value}


def proof_records(n):
    require(n in (0,100,1000,10000),"unknown frozen dataset")
    for index in range(n):
        fields={"title":tag("string",f"東京🌿-{index}"),"score":tag("integer",str(index)),
                "ratio":tag("float",f"{index}.25"),"active":tag("boolean",index%2==0),
                "at":tag("timestamp",str(1700000000000+index)),
                "author_id":tag("reference",f"author:{index//10:06}")}
        if index%2:
            fields["note"]=tag("string",f"note-{index}")
        yield {"table":"articles","row":{"id":f"article:{index:06}","fields":fields}}
    for index in range(n//10):
        yield {"table":"authors","row":{"id":f"author:{index:06}","fields":{"name":tag("string",f"author-{index:06}")}}}
    for table in ("tags","article_tags"):
        yield {"table":table,"row":None}


def _pairs(pairs):
    result={}
    for key,value in pairs:
        require(key not in result,"duplicate JSON key")
        result[key]=value
    return result


def strict_json(value):
    def invalid(_):
        raise RunnerError("non-finite JSON")
    try:
        return json.loads(value,object_pairs_hook=_pairs,parse_constant=invalid)
    except (ValueError,UnicodeError,TypeError) as error:
        raise RunnerError("invalid JSON record") from error


class ProofCapture:
    MAX_LINE=1048576+7
    def __init__(self,n):
        require(n in (0,100,1000,10000),"unknown frozen dataset")
        self.n=n
        self.phase=None
        self.expected=None
        self.digest=None
        self.count=0
        self.proof={}
        self.report=None
        self.resource=None
        self.ready=False
        self.rss=[]

    def feed(self,line):
        require(isinstance(line,bytes) and len(line)<=self.MAX_LINE and line.endswith(b"\n"),"unbounded or incomplete worker line")
        if line.startswith(b"RSS "):
            require(self.phase is None and self.report is None,"RSS during proof/after report")
            checkpoint=strict_json(line[len(b"RSS "):-1])
            index=len(self.rss)
            expected={"phase":"baseline","cycle":None} if index==0 else {"phase":"after_open" if index%2 else "after_close","cycle":(index-1)//2}
            require(index<61 and checkpoint==expected and canonical(checkpoint)==canonical(expected),"invalid RSS sequence")
            if index==0:require(not self.proof,"RSS baseline after proof")
            if index==2:require("initial" in self.proof,"missing initial RSS fixture proof")
            if index==60:require("final" in self.proof,"missing final RSS fixture proof")
            self.rss.append(checkpoint)
        elif line==b"READY\n":
            require(not self.ready and self.phase is None and not self.proof and self.report is None,"invalid ready phase")
            self.ready=True
        elif line.startswith(b"PROOF_BEGIN "):
            phase=line[len(b"PROOF_BEGIN "):-1].decode("ascii")
            required="initial" if not self.proof else "final"
            require(phase==required and self.phase is None and len(self.proof)<2 and self.report is None,"invalid proof phase")
            if self.rss:require(len(self.rss)==(2 if phase=="initial" else 60),"RSS proof at wrong cycle")
            self.phase=phase
            self.expected=iter(proof_records(self.n))
            self.digest=hashlib.sha256()
            self.count=0
        elif line.startswith(b"PROOF "):
            require(self.phase is not None,"proof outside phase")
            raw=line[len(b"PROOF "):-1]
            value=strict_json(raw)
            require(raw.decode("utf-8")==canonical(value),"noncanonical proof record")
            expected=next(self.expected,None)
            require(expected is not None and canonical(value)==canonical(expected),"wrong typed proof value/order")
            self.digest.update(raw+b"\n")
            self.count+=1
        elif line.startswith(b"PROOF_END "):
            phase=line[len(b"PROOF_END "):-1].decode("ascii")
            require(phase==self.phase and self.phase is not None and next(self.expected,None) is None,"incomplete proof")
            self.proof[phase]={"count":self.count,"sha256":self.digest.hexdigest()}
            self.phase=None
            self.expected=None
            self.digest=None
        elif line.startswith(b"REPORT "):
            require(self.phase is None and set(self.proof)=={"initial","final"} and self.report is None and (not self.rss or len(self.rss)==61),"report before complete proof/RSS")
            report=strict_json(line[len(b"REPORT "):-1])
            require(isinstance(report,dict) and report.get("format")==1,"invalid report")
            self.report=report
        elif line.startswith(b"RESOURCE "):
            require(self.report is not None and self.resource is None,"invalid resource phase")
            self.resource=strict_json(line[len(b"RESOURCE "):-1])
        else:
            raise RunnerError("unexpected worker output")

    def finish(self):
        require(self.phase is None and set(self.proof)=={"initial","final"} and self.report is not None,"incomplete worker output")
        result={"report":self.report,"proof":self.proof}
        if self.ready:
            result["ready"]=True
        if self.rss:
            require(len(self.rss)==61,"incomplete RSS checkpoints")
            result["rss_checkpoints"]=self.rss
        if self.resource is not None:
            result["resource"]=self.resource
        return result


def audit_locks(root,standalone):
    def key(package):
        return tuple(package.get(field) for field in ("name","version","source","checksum"))
    require(isinstance(root,dict) and isinstance(standalone,dict),"invalid lock")
    approved={key(package) for package in root.get("package",[])}
    require(approved,"empty approved lock")
    packages=standalone.get("package",[])
    require(isinstance(packages,list) and packages,"empty standalone lock")
    require(sum(key(package)==("hoyadb-benchmark","0.0.0",None,None) for package in packages)==1,"missing/duplicate worker package")
    for package in packages:
        if key(package)==("hoyadb-benchmark","0.0.0",None,None):
            continue
        require(key(package) in approved,"new/unapproved package identity")
    return sum("source" in package for package in standalone.get("package",[]))


def execute_worker(command,n,output,rss=False,store_directory=None):
    """Capture bounded proof lines; retain failures, never fabricate sample values."""
    output=Path(output)
    with output.open("x",encoding="utf-8") as destination, tempfile.TemporaryFile() as errors:
        capture=ProofCapture(n)
        start=time.monotonic_ns()
        process=None
        result={"format":1,"status":"failed"}
        try:
            process=subprocess.Popen(command,stdin=subprocess.PIPE if rss else subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=errors)
            result["pid"]=process.pid
            failure=None
            try:
                while line:=process.stdout.readline(ProofCapture.MAX_LINE+1):
                    require(rss or not line.startswith(b"RSS "),"unexpected RSS mode")
                    capture.feed(line)
                    if line.startswith(b"RSS "):
                        point=capture.rss[-1]
                        measurement={**point,**sample_rss(process.pid)}
                        if store_directory is not None and point["cycle"] is not None:
                            measurement["store"]=store_sizes(Path(store_directory)/f"cycle-{point['cycle']:02}")
                            measurement["store"]["state"]=point["phase"]
                        result.setdefault("rss_observations",[]).append(measurement)
                        process.stdin.write(b"CONTINUE\n")
                        process.stdin.flush()
                    if line==b"READY\n":
                        result["launch_to_parent_observed_ready_ns"]=time.monotonic_ns()-start
                result.update(capture.finish())
            except (RunnerError,UnicodeError):
                failure="invalid/incomplete worker protocol"
                if hasattr(os,"wait4"):
                    try:
                        os.kill(process.pid,signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                else:
                    process.kill()
            finally:
                process.stdout.close()
                if rss:process.stdin.close()
            if hasattr(os,"wait4"):
                pid,status,usage=os.wait4(process.pid,0)
                require(pid==process.pid,"wrong reaped worker")
                process.returncode=os.waitstatus_to_exitcode(status)
                result["resource"]={"tool":"os.wait4 owned child", "ru_maxrss":usage.ru_maxrss,
                    "ru_maxrss_unit":"bytes" if sys.platform=="darwin" else "unknown",
                    "peak_rss_bytes":usage.ru_maxrss if sys.platform=="darwin" else None,
                    "user_cpu_seconds":usage.ru_utime,"system_cpu_seconds":usage.ru_stime}
            else:
                process.wait()
                result["resource"]={"tool":"unavailable","peak_rss_bytes":None}
            result["exit_code"]=process.returncode
            if not failure and process.returncode:
                failure="worker exited unsuccessfully"
            if failure:
                result["failure"]=failure
                result.setdefault("proof",capture.proof)
                if capture.report is not None:
                    result.setdefault("report",capture.report)
            else:
                result["status"]="ok"
        except OSError:
            result["failure"]="worker launch/system failure"
            if process is not None and process.returncode is None:
                process.kill()
                process.wait()
                result["exit_code"]=process.returncode
        finally:
            result["launch_to_exit_ns"]=time.monotonic_ns()-start
            errors.seek(0)
            stderr=errors.read(65537)
            result["stderr"]=stderr[:65536].decode("utf-8",errors="replace")
            result["stderr_truncated"]=len(stderr)>65536
            destination.write(canonical(result)+"\n")
    return result


class Admission:
    """One-use parent permits between exactly reaped workers (benchmarks/admission-plan.md).

    Waiting happens before the next child's launch clock and after the previous
    child's wait4 reap and validated raw persistence; it never enters a sample clock.
    """
    CONTINUITY="thermal/cache/environment continuity across independent workers is uncontrolled"

    def __init__(self,fd,announce,recheck,events):
        self.fd=fd
        self.announce=announce
        self.recheck=recheck
        self.events=events
        self.buffer=b""
        self.held=None

    def record(self,event,**fields):
        value={"event":event,"monotonic_ns":time.monotonic_ns(),**fields}
        self.events.append(value)
        return value

    def reject(self,reason):
        self.record("rejected",reason=reason)
        raise RunnerError(reason)

    def pending(self):
        import select
        return bool(self.buffer) or bool(select.select([self.fd],[],[],0)[0])

    def line(self):
        while b"\n" not in self.buffer:
            if len(self.buffer)>65536:
                self.reject("unbounded control line")
            chunk=os.read(self.fd,4096)
            if not chunk:
                self.reject("control input closed")
            self.buffer+=chunk
        line,self.buffer=self.buffer.split(b"\n",1)
        try:
            return line.decode("utf-8")
        except UnicodeError:
            self.reject("malformed control input")

    def admit(self,coordinates):
        if self.held is None and self.pending():
            self.reject("pre-issued admission before announcement")
        expected="ADMIT "+canonical(coordinates)
        self.announce("AWAIT_ADMISSION "+canonical(coordinates))
        while True:
            line=self.line()
            if line.startswith("DECLARE ") and self.held is not None:
                try:
                    declaration=strict_json(line[len("DECLARE "):])
                except RunnerError:
                    self.reject("malformed competing-work declaration")
                if not isinstance(declaration,dict) or len(line)>4096:
                    self.reject("malformed competing-work declaration")
                self.record("declare",declaration=declaration,hold=self.held)
                continue
            if line.startswith("CANCEL "):
                self.record("cancelled",reason=line[len("CANCEL "):][:4096],hold=self.held)
                raise RunnerError("cancelled by parent")
            if line!=expected:
                self.reject("unexpected, stale or malformed admission")
            break
        if not self.recheck():
            self.reject("frozen source/artifact drift before admission")
        self.record("admit",coordinates=coordinates,hold=self.held,continuity=self.CONTINUITY)
        self.held=None

    def hold(self,label,raw_path,pid):
        # Anything already sent was written before the held state could be observed.
        if self.pending():
            self.reject("pre-issued admission before held state")
        self.held={"label":label,"pid":pid,"raw_sha256":file_hash(raw_path),
                   "worker_retirement":"os.wait4 exact child reaped" if hasattr(os,"wait4") else "Popen.wait exact child reaped",
                   "continuity":self.CONTINUITY}
        self.held["monotonic_ns"]=self.record("between_workers",**self.held)["monotonic_ns"]
        self.announce("BETWEEN_WORKERS "+canonical(self.held))
        return dict(self.held)


def sample_rss(pid):
    command=["ps","-o","rss=","-p",str(pid)]
    observed=observation(command)
    text=observed.get("text","").strip()
    raw=int(text) if text.isascii() and text.isdigit() else None
    known=raw is not None and sys.platform=="darwin"
    return {"tool":"ps -o rss= -p owned_pid","pid":pid,"observed_monotonic_ns":time.monotonic_ns(),
            "raw_value":raw,"raw_unit":"1024 bytes" if known else "unknown",
            "resident_bytes":raw*1024 if known else None}


def source_snapshot(root,paths):
    root=Path(root).resolve(strict=True)
    selected={}
    for name in paths:
        path=root/name
        require(path.exists() and not path.is_symlink(),"missing/symlink source input")
        candidates=path.rglob("*") if path.is_dir() else [path]
        for candidate in candidates:
            relative=candidate.relative_to(root)
            if any(part in {"target","__pycache__",".git","node_modules"} for part in relative.parts):
                continue
            require(not candidate.is_symlink(),"symlink source input")
            if candidate.is_file() and (candidate.suffix in {".rs",".py",".toml",".json",".lock",".md"} or candidate.name=="Cargo.lock"):
                selected[relative.as_posix()]=hashlib.sha256(candidate.read_bytes()).hexdigest()
    require(selected,"empty source identity")
    selected=dict(sorted(selected.items()))
    return {"sha256":hashlib.sha256(canonical(selected).encode()).hexdigest(),"files":selected}


def checked_report(raw,identity,frozen,current):
    require(frozen==current,"source changed during experiment")
    require(raw.get("status")=="ok" and raw.get("exit_code")==0,"failed raw worker")
    proof=raw.get("proof",{})
    require(set(proof)=={"initial","final"} and proof["initial"]==proof["final"],"missing/mismatched independent row proof")
    report=raw.get("report",{})
    require(isinstance(report,dict) and isinstance(report.get("identity"),dict),"missing raw report")
    require(all(report["identity"].get(key)==value for key,value in identity.items()),"worker identity changed")
    return report


SOURCE_PATHS=["Cargo.toml","Cargo.lock","rust-toolchain.toml","rustfmt.toml","crates",
              "benchmarks/native","benchmarks/run.py","benchmarks/summary.py","benchmarks/workloads.json",
              "benchmarks/PLAN.md","benchmarks/test_runner.py","benchmarks/test_summary.py"]


def file_hash(path):
    digest=hashlib.sha256()
    with Path(path).open("rb") as source:
        while part:=source.read(1048576):
            digest.update(part)
    return digest.hexdigest()


def write_new(path,value):
    with Path(path).open("x",encoding="utf-8") as destination:
        destination.write(canonical(value)+"\n")


def observation(command):
    try:
        result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20,check=False)
        if result.returncode:
            return {"status":"unknown","returncode":result.returncode}
        return {"status":"observed","text":result.stdout[:65536].decode("utf-8",errors="replace"),"truncated":len(result.stdout)>65536}
    except (OSError,subprocess.TimeoutExpired):
        return {"status":"unknown"}


def environment():
    info=time.get_clock_info("monotonic")
    result={"os":platform.platform(),"machine":platform.machine(),"python":platform.python_version(),
            "logical_cpus":os.cpu_count(),"clock":{"implementation":info.implementation,"monotonic":info.monotonic,
            "adjustable":info.adjustable,"nominal_resolution_seconds":info.resolution},
            "cpu":"unknown","memory":"unknown","power":"unknown","thermal":"unknown","background":"unknown"}
    if sys.platform=="darwin":
        result.update(cpu=observation(["sysctl","-n","machdep.cpu.brand_string"]),memory=observation(["sysctl","-n","hw.memsize"]),
                      power=observation(["pmset","-g","batt"]),thermal=observation(["pmset","-g","therm"]))
    return result


def store_sizes(directory):
    result={}
    for path in sorted(Path(directory).iterdir()):
        require(not path.is_symlink(),"unexpected symlink in owned store")
        require(path.is_file(),"unexpected directory in owned store")
        result[path.name]=path.stat().st_size
    return {"state":"after worker closed/exited","regular_files_bytes":result,"total_bytes":sum(result.values())}


def cold_cohorts(workload,engine):
    for declaration in workload["cold"]:
        if engine not in declaration["engines"]:
            continue
        sizes=declaration["N"] if isinstance(declaration["N"],list) else [declaration["N"]]
        for n in sizes:
            yield {"operation":declaration["operation"],"N":n,"Q":0,"observers":0}


def arguments(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",required=True,type=Path)
    parser.add_argument("--engine",choices=["memory","native-protected","both"],default="both")
    parser.add_argument("--scope",choices=["warm","cold","resources","all"],default="all")
    parser.add_argument("--cohort",type=int,action="append",help="explicit zero-based warm cohort subset, unchanged samples")
    parser.add_argument("--run-status",choices=["ok","contaminated"],default="contaminated",help="ok requires independently quiet caller-approved window")
    parser.add_argument("--admission",choices=["none","stdin"],default="none",help="stdin requires an exact ADMIT line before every worker launch")
    args=parser.parse_args(argv)
    if args.run_status=="ok" and args.admission!="stdin":
        # A07: reject before any build, output directory, fixture or child acquisition.
        parser.error("--run-status ok requires --admission stdin")
    return args


def main():
    from summary import summarize,InvalidSamples
    args=arguments()
    root=Path(__file__).resolve().parents[1]
    output=args.output.resolve()
    output.mkdir(parents=True,exist_ok=False)
    manifest={"format":1,"status":"running","run_status":args.run_status,"quiet_window_attested_by_caller":args.run_status=="ok",
              "selection":{"engine":args.engine,"scope":args.scope,"warm_cohorts":args.cohort},"started_unix_ns":time.time_ns(),
              "environment_before":environment(),"unsupported_metrics":["browser JS heap/WASM memory","mobile package metrics"]}
    events=[]
    manifest["admission"]={"mode":args.admission,"events":events,"continuity":Admission.CONTINUITY if args.admission=="stdin" else None}
    summaries=[]
    try:
        workload=strict_json((root/"benchmarks/workloads.json").read_bytes())
        frozen=source_snapshot(root,SOURCE_PATHS)
        manifest["source_before"]=frozen
        with (root/"Cargo.lock").open("rb") as source:
            approved=tomllib.load(source)
        with (root/"benchmarks/native/Cargo.lock").open("rb") as source:
            standalone=tomllib.load(source)
        manifest["approved_external_packages"]=audit_locks(approved,standalone)
        manifest["locks"]={name:file_hash(root/name) for name in ["Cargo.lock","benchmarks/native/Cargo.lock"]}
        command=["cargo","build","--release","--manifest-path","benchmarks/native/Cargo.toml","--offline","--locked"]
        manifest["build_command"]=command
        manifest["compiler"]=observation(["rustc","-Vv"])
        with (output/"build.txt").open("xb") as log:
            built=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT,check=False)
        manifest["build_exit_code"]=built.returncode
        require(built.returncode==0,"release build failed")
        require(source_snapshot(root,SOURCE_PATHS)==frozen,"source changed during build")
        executable=root/"benchmarks/native/target/release/hoyadb-benchmark"
        artifact=file_hash(executable)
        manifest["artifact"]={"path":str(executable.relative_to(root)),"sha256":artifact,"bytes":executable.stat().st_size,"profile":"release","features":"ordinary defaults"}
        workload_hash=file_hash(root/"benchmarks/workloads.json")
        admission=None
        if args.admission=="stdin":
            admission=Admission(sys.stdin.fileno(),lambda line:print(line,flush=True),
                                lambda:source_snapshot(root,SOURCE_PATHS)==frozen and file_hash(executable)==artifact,events)
        manifest["workload_sha256"]=workload_hash
        engines=["memory","native-protected"] if args.engine=="both" else [args.engine]
        selected=range(len(workload["cohorts"])) if args.cohort is None else args.cohort
        require(all(0<=index<len(workload["cohorts"]) for index in selected),"invalid cohort index")
        require(len(set(selected))==len(selected),"duplicate cohort index")
        with tempfile.TemporaryDirectory(prefix="hoya-benchmark-") as temporary:
            owned=Path(temporary).resolve()
            manifest["owned_temp_directory"]=str(owned)
            fixtures={}
            def identity(engine,cohort,endpoint):
                return {"class":"rust-facade/"+engine,"profile":"release","unit":"ns","endpoint":endpoint,
                        "artifact_sha256":artifact,"workload_sha256":workload_hash,"source_sha256":frozen["sha256"],"parameters":cohort}
            def execute(engine,cohort,run,kind,directory,label):
                # DEF-0020: the reactive worker's primary value is its last consumer callback clock.
                warm="caller-harness-invocation-to-last-callback" if cohort["operation"]=="reactivity" else "serialize/facade-call/decoded-return"
                expected=identity(engine,cohort,warm if kind=="warm" else "untimed-open-observe-close-resources" if kind=="resources" else "serialize/protected-open/decoded-owner" if engine=="native-protected" else "serialize/memory-open/decoded-owner")
                if kind=="resources":expected["unit"]="bytes"
                config={"cohort":cohort,"directory":str(directory) if directory else None,"engine":engine,"identity":expected,"run":run,"run_status":args.run_status}
                config_path=output/(label+"-config.json")
                write_new(config_path,config)
                flag={"warm":"--config","cold":"--cold-config","setup":"--prepare-config","resources":"--rss-config"}[kind]
                if admission is not None:
                    # Parent wait is before this child's launch clock starts.
                    admission.admit({"label":label,"kind":kind,"engine":engine,"run":run,"parameters":cohort})
                raw=execute_worker([str(executable),flag,str(config_path)],cohort["N"],output/(label+"-raw.json"),rss=kind=="resources",store_directory=directory)
                if directory:
                    if kind=="resources":
                        inventory={}
                        for path in sorted(directory.iterdir()):
                            require(path.is_dir() and not path.is_symlink(),"invalid owned RSS cycle directory")
                            inventory[path.name]=store_sizes(path)
                        write_new(output/(label+"-store.json"),inventory)
                    else:
                        write_new(output/(label+"-store.json"),store_sizes(directory))
                require(file_hash(executable)==artifact,"artifact changed during experiment")
                report=checked_report(raw,expected,frozen,source_snapshot(root,SOURCE_PATHS))
                if kind=="cold":
                    require(raw.get("ready") and raw.get("launch_to_parent_observed_ready_ns",0)>0,"missing cold readiness observation")
                if admission is not None:
                    admission.hold(label,output/(label+"-raw.json"),raw["pid"])
                return report,raw
            if args.scope in {"warm","all"}:
                for engine in engines:
                    for index in selected:
                        cohort=workload["cohorts"][index]
                        label=f"warm-{engine}-{index:02}"
                        reports=[]
                        for run in range(1,workload["sampling"]["independent_runs"]+1):
                            directory=None
                            if engine=="native-protected":
                                directory=owned/f"{label}-{run}";directory.mkdir()
                            report,_=execute(engine,cohort,run,"warm",directory,f"{label}-{run}")
                            reports.append(report)
                            print(f"completed {label} run {run}",flush=True)
                        if args.run_status=="ok":
                            summaries.append((label,summarize(reports)))
            if args.scope in {"cold","all"}:
                for engine in engines:
                    for index,cohort in enumerate(cold_cohorts(workload,engine)):
                        label=f"cold-{engine}-{index:02}"
                        reports=[];outer=[]
                        for run in range(1,workload["sampling"]["cold_processes"]+1):
                            directory=None
                            if engine=="native-protected":
                                directory=owned/f"{label}-{run}"
                                if cohort["operation"]=="create":
                                    directory.mkdir()
                                else:
                                    n=cohort["N"]
                                    if n not in fixtures:
                                        fixture=owned/f"fixture-{n}";fixture.mkdir()
                                        seed_cohort={"operation":"point","N":n,"Q":0,"observers":0}
                                        execute(engine,seed_cohort,1,"setup",fixture,f"setup-{n}")
                                        fixtures[n]=fixture
                                    shutil.copytree(fixtures[n],directory)
                            report,raw=execute(engine,cohort,run,"cold",directory,f"{label}-{run}")
                            reports.append(report)
                            item=json.loads(canonical(report))
                            item["identity"]["endpoint"]="process-launch/parent-observed-decoded-ready"
                            item["runs"][0]["samples"][0]["value"]=raw["launch_to_parent_observed_ready_ns"]
                            outer.append(item)
                        if args.run_status=="ok":
                            summaries.extend([(label+"-inner",summarize(reports)),(label+"-outer",summarize(outer))])
            if args.scope in {"resources","all"}:
                for engine in engines:
                    directory=None
                    if engine=="native-protected":
                        directory=owned/"resources-native-protected";directory.mkdir()
                    cohort={"operation":"retained-rss","N":100,"Q":0,"observers":1}
                    report,raw=execute(engine,cohort,1,"resources",directory,f"resources-{engine}")
                    points=raw.get("rss_observations",[])
                    require(len(points)==61,"missing RSS observations")
                    baseline=points[0].get("resident_bytes")
                    final=points[-1].get("resident_bytes")
                    metric={"format":1,"kind":"untimed-resources","status":args.run_status,"identity":report["identity"],
                            "tool":"ps -o rss= -p owned_pid; os.wait4 exact child","cycles":30,
                            "baseline_resident_bytes":baseline,"final_post_close_resident_bytes":final,
                            "retained_delta_bytes":final-baseline if baseline is not None and final is not None else None,
                            "peak_rss":raw["resource"],"interpretation":"retained process delta; not an allocator leak conclusion"}
                    write_new(output/f"resources-{engine}-metric.json",metric)
            manifest["owned_store_cleanup"]="temporary directory removed after all children reaped"
        manifest["source_after"]=source_snapshot(root,SOURCE_PATHS)
        require(manifest["source_after"]==frozen,"source changed during experiment")
        require(file_hash(executable)==artifact,"artifact changed during experiment")
        for label,summary in summaries:
            write_new(output/(label+"-summary.json"),summary)
        manifest["accepted_summaries"]=len(summaries)
        manifest["status"]="ok" if args.run_status=="ok" else "contaminated"
        returncode=0
    except (RunnerError,InvalidSamples,OSError,ValueError) as error:
        manifest["status"]="failed"
        manifest["failure"]=str(error)
        returncode=1
    finally:
        manifest["finished_unix_ns"]=time.time_ns()
        manifest["environment_after"]=environment()
        write_new(output/"manifest.json",manifest)
    return returncode


if __name__=="__main__":
    raise SystemExit(main())
