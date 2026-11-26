# Public artifact validation tools

This directory is a deliberately limited extraction of the dependency-free
Markdown artifact checker, RTM generator and regression fixtures. It contains no
private framework rules, templates, reference examples or agent skills. Python's
standard library is sufficient. [provenance.json](provenance.json) records the
original extraction's source revision and SHA-256 hashes. Those historical hashes
remain unchanged; `trace_check.py` and `rtm_gen.py` now include the input-validation
fix in [DEF-0052](../../docs/design/defects/DEF-0052-traceability-input-errors.md).
The original regression test and fixtures retain their extracted bytes; the new
input-error cases live in [test_input_errors.py](test_input_errors.py).

```sh
python3 -m unittest discover -s tools/traceability -p 'test_*.py'
python3 -m unittest discover -s tests/packaging -p 'test_*.py'
python3 tools/traceability/trace_check.py docs/design
python3 tools/traceability/rtm_gen.py docs/design --check
```

Generate the RTM with the same command without `--check`. The checker validates
required frontmatter, status values, IDs, link targets (including mitigation
targets) and accepted requirement/NFR coverage. It does not check whether every
threat has sufficient mitigation. Its parser intentionally accepts only the restricted
frontmatter subset used in the existing artifacts. It is not a general YAML
parser, registry allocation validator or ordinary Markdown hyperlink checker.

Both tools reject a missing or non-directory scan root, failed directory scans,
unreadable or non-UTF-8 Markdown input, and an opening frontmatter delimiter with
no closing delimiter. The checker reports `input-invalid`; the RTM generator
exits unsuccessfully before writing output. Ordinary Markdown without leading
frontmatter is allowed. An existing, readable empty directory also remains valid:
zero artifacts is not a claim that the project has complete requirements. The
normal project command must point at `docs/design`.

Scan `docs/design` only. The deliberately invalid fixtures under `testdata` must
not enter the product graph. Historical private spec/rule IDs in implementation
comments describe provenance; contributors need only the public
[contribution contract](../../CONTRIBUTING.md) and checked-in project artifacts.
This extraction does not select the project's distribution license.
