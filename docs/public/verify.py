"""SPEC-0041 / TC-0045: execute all documentation examples, fail on any error."""
import hashlib
import json
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
REPORT = HERE / 'verification.json'


def source_hashes():
    paths = set()
    for base in ['examples', 'crates', 'crates/hoyadb-web/src', 'packages/client/src', 'packages/testing/src',
                 'bindings/swift/Sources', 'crates/hoyadb-native/src', 'crates/hoyadb-native/android', 'bindings/android/src', 'bindings/android/tools', 'packages/react-native/src', 'docs/public']:
        paths.update(p for p in (REPO/base).rglob('*') if p.is_file() and p.suffix in {'.rs','.js','.mjs','.py','.html','.css','.toml','.lock','.ts','.swift','.kt','.java','.c','.h','.json'}
                     and p.name not in {'verification.json','browser-check.json'}
                     and not any(part in {'dist','target','__pycache__','.build'} for part in p.relative_to(REPO/base).parts))
    for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/hoyadb-web/Cargo.toml','crates/hoyadb-web/Cargo.lock','docs/public/site.json','docs/public/symbols.json','crates/hoyadb-native/Cargo.toml','crates/hoyadb-native/Cargo.lock','bindings/swift/Package.swift',
                 'packages/client/package.json','packages/testing/package.json',
                 'tools/javascript/scripts/build.mjs','tools/javascript/scripts/application-browser.mjs',
                 'tests/packaging/javascript/application-row-types.ts','bindings/android/toolchain-review.json']:
        path = REPO/name
        if path.exists(): paths.add(path)
    for pattern in ['crates/hoyadb-native/include/*.h', 'packages/client/*.d.ts', 'packages/testing/*.d.ts', 'packages/react-native/*.ts']:
        paths.update(REPO.glob(pattern))
    paths.add(REPO/'crates/hoyadb-web/build.sh')
    for path in (REPO/'crates/hoyadb-web/pkg').glob('*'):
        if path.suffix in {'.js','.wasm','.ts'}: paths.add(path)
    for filename, fields in [('native-build.json', ('artifacts','swift_inputs')),
                             ('kotlin/manifest.json', ('artifacts','inputs'))]:
        native_report = REPO/'target/documentation-languages'/filename
        if native_report.is_file():
            paths.add(native_report)
            data = json.loads(native_report.read_text())
            for field in fields:
                for rel in data.get(field, {}):
                    artifact = (REPO/rel).resolve()
                    guide_input = field == 'inputs' and artifact == (REPO/'examples/kotlin/TodoGuide.kt').resolve()
                    if (not artifact.is_relative_to(REPO/'target/documentation-languages') and not guide_input) or not artifact.is_file():
                        raise ValueError(f'unsafe or missing native documentation input/artifact: {rel}')
                    paths.add(artifact)
    return {p.relative_to(REPO).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}


def main():
    REPORT.unlink(missing_ok=True)
    commands = []
    env = dict(os.environ, CARGO_TARGET_DIR=str(REPO/'target/public-documentation/examples'))
    def run(name, args, custom_env=None, mode="execution"):
        print('Running', name, flush=True)
        result = subprocess.run(args, cwd=REPO, env=custom_env, capture_output=True, text=True)
        commands.append({'name': name, 'command': ' '.join(args), 'exit_code': result.returncode,
                         'mode':mode, 'environment':({'HOYADB_CHROME':os.environ['HOYADB_CHROME']} if name == 'application-browser' and 'HOYADB_CHROME' in os.environ else {}), 'output': (result.stdout+result.stderr).replace(str(REPO), '<checkout>')})
        log = REPO/'target/public-documentation'/f'{name}.txt'
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text(result.stdout+result.stderr)
        if result.returncode:
            print(result.stdout+result.stderr)
            raise SystemExit(f'FAIL {name}; no passing report written')
        print('PASS', name, flush=True)
    run('tooling', ['python3','-m','unittest','discover','-s','docs/public','-p','test_*.py'])
    run('wasm-build', ['sh','crates/hoyadb-web/build.sh'])
    run('javascript-package', ['node','tools/javascript/scripts/build.mjs'])
    run('native-build', ['python3','docs/public/run_native.py','build'])
    run('kotlin-build', ['python3','docs/public/run_kotlin.py','build'])
    # Hash after generation, before executing any example. No source changes
    # during the run may be labelled as tested by this report.
    before = source_hashes()
    run('rust-build', ['cargo','build','--manifest-path','examples/Cargo.toml','--offline','--locked'], env)
    run('rust-command-tests', ['cargo','test','--manifest-path','examples/Cargo.toml','--bin','commands','--offline','--locked'], env)
    for path in sorted((REPO/'examples/src/bin').glob('*.rs')):
        run(path.stem, ['cargo','run','--manifest-path','examples/Cargo.toml','--bin',path.stem,'--offline','--locked'], env)
    run('javascript', ['node','examples/javascript.mjs'])
    run('application', ['node','docs/public/run_application.mjs'])
    run('typescript', ['node','docs/public/run_language.mjs','typescript'])
    run('react-native', ['node','docs/public/run_language.mjs','react-native'])
    run('react-native-bootstrap', ['python3','docs/public/check_sources.py','react-native-bootstrap'], mode='source-check')
    run('kotlin-source', ['python3','docs/public/check_sources.py','kotlin'], mode='source-check')
    run('kotlin', ['python3','docs/public/run_kotlin.py','run'])
    run('swift-format', ['xcrun','swift-format','lint','--strict','examples/swift/Sources/TodoGuide/main.swift','examples/swift/Package.swift'])
    run('swift', ['python3','docs/public/run_native.py','swift'])
    run('c', ['python3','docs/public/run_native.py','c'])
    run('application-browser', ['node','tools/javascript/scripts/application-browser.mjs'])
    run('example-lint', ['cargo','clippy','--manifest-path','examples/Cargo.toml','--all-targets','--offline','--locked','--','-D','warnings'], env)
    run('search', ['node','docs/public/test_search.mjs'])
    after = source_hashes()
    if before != after:
        raise SystemExit('FAIL sources changed during verification; rerun on the final files')
    report = {'commit': subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip(),
              'date': datetime.now(timezone.utc).isoformat(), 'source_hashes': after, 'commands': commands,
              'native_build':json.loads((REPO/'target/documentation-languages/native-build.json').read_text()),
              'kotlin_build':json.loads((REPO/'target/documentation-languages/kotlin/manifest.json').read_text()),
              'kotlin_run':json.loads((REPO/'target/documentation-languages/kotlin/run.json').read_text()),
              'limits': ['Rust, JavaScript, strict emitted TypeScript, Swift and C examples executed; application browser/worker/OPFS fixture also executed.',
                         'React Native shared logic executed against WASM; native bootstrap not compiled or executed. Kotlin executed on an Android emulator via app_process, not an APK build or device certification; no D1/D2, Phase 7 or R9 gate claim.',
                         'Native Swift/C and Kotlin use an archived HEAD core with a task-only standalone workspace stanza; Rust/WASM examples use recorded working-tree bytes. Pre-existing edits are preserved.']}
    REPORT.write_text(json.dumps(report, indent=2)+'\n')
    print(f'PASS {len(commands)} commands; source hashes unchanged; {REPORT}')


if __name__ == '__main__':
    main()
