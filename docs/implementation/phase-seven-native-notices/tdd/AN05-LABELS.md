# AN05 TDD step labels

| Step | Kind | What it shows |
| --- | --- | --- |
| an05-red-1 | setup failure, not a behavioral RED | 11 errors: the fixture copies the shipped android/.npmignore, which did not exist yet |
| an05-red-2 | setup failure, not a behavioral RED | 5 failures: pack.py rejected the new --package-root/--repo-root arguments (seam absent) |
| an05-red-3-seam-only | behavioral RED | with only the root seam added, untracked, tracked-modified and stray archives were packed and accepted (exit 0); wrong root gave io-error; no source_identity record |
| an05-green-1 | one ordering failure | wrong root gave io-error because validator code loaded from repo_root before the Git-root check |
| an05-green-2 | GREEN | 50/50 in -I and -I -O; frozen as cut-50 (handback; root re-ran it in root-verification) |
| an05-red-4-commit-pin | behavioral RED (see that step) | literal HEAD used for member checks after pinning head_commit; inherited GIT_* overrides explicit roots |
