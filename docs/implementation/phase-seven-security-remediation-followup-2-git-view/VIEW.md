# Git-ready derived view of `phase-seven-security-remediation-followup-2`

This directory is a DERIVED view for version control. It is **not** byte-identical
to the frozen cut: 3 file(s) are omitted and recorded in
[OMISSIONS.json](OMISSIONS.json) with their original SHA256, size, the reason
for omission and the official upstream location. The frozen cut, including its
original `files-sha256.json` (sha256 `fc60b72548959e3047e793b072fc7166005fe41ec85e04d853d97556ab96e724`),
is preserved byte-exact only locally under the ignored
`target/phase-seven-security-remediation/frozen-cuts/phase-seven-security-remediation-followup-2/`.

Root decision A (follow-up 5): the guard now uses an independently written
`java_properties.py` implemented from the public `Properties.load(Reader)` API.
The superseded translation and the JDK source excerpts are not tracked. Every
other file is copied unchanged; `view-files-sha256.json` indexes this view.
No legal clean-room certification is claimed.
