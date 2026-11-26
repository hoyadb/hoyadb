"""Source-only quickstart checks. This does not compile or execute a binding."""
import json
import sys
from pathlib import Path
import build
from source_checks import check_calls, check_status

name = sys.argv[1]
entry = json.loads((build.HERE/'language-examples.json').read_text())[name]
if not check_status(entry):
    raise SystemExit('invalid example execution/source-check status')
source = build.REPO/'examples'/entry['path']
if not source.is_file(): raise SystemExit(f'missing source: {source}')
regions = build.example_regions(source.read_text())
if not regions: raise SystemExit(f'no included regions: {source}')
guide = {'kotlin':'kotlin-getting-started', 'react-native-bootstrap':'react-native-getting-started'}[name]
inventory = json.loads((build.HERE/'symbols.json').read_text())[guide]
errors = build.check_symbols({guide:inventory}, build.REPO)
if errors: raise SystemExit('\n'.join(errors))
# The inventory must name actual symbols used by the snippet as well as declarations.
import re
for row in inventory:
    for symbol in row['names']:
        if not re.search(r'\b'+re.escape(symbol)+r'\b', source.read_text()):
            # RN shares the rest of its logic in the separately executed file.
            shared = (build.REPO/'examples/react-native.mjs').read_text() if name == 'react-native-bootstrap' else ''
            if not re.search(r'\b'+re.escape(symbol)+r'\b', shared):
                raise SystemExit(f'{source}: checked symbol not used: {symbol}')

if name == 'kotlin':
    declarations = '\n'.join(path.read_text() for path in (build.REPO/'bindings/android/src').rglob('*.kt'))
    allowlist = {'Executor','execute','run','println','require','check','error','listOf','mapOf','emptyMap','emptyList','String','Boolean','Long','Unit','mutableListOf','forEach','contains','isNotEmpty','isEmpty','size','add',
                 'Seen','record',  # Local observer recorder declared in the checked guide.
                 'shutdown'}  # JDK ExecutorService cleanup.
    lang = 'kotlin'
else:
    declarations = (build.REPO/'packages/react-native/src/index.js').read_text() + (build.REPO/'packages/client/application.d.ts').read_text()
    allowlist = {'runTodo'}
    lang = 'javascript'
unknown = check_calls(source.read_text(), declarations, allowlist, lang)
if unknown: raise SystemExit('\n'.join(f'{source}: {error}' for error in unknown))

print(f'PASS {name}: checked-in regions and named symbols match current declarations')
if entry['status'] == 'source-checked':
    print(f'Source-checked · not compiled or executed: {entry["reason"]}')
else:
    print('Additional source check only; compilation and runtime proof are recorded separately')
