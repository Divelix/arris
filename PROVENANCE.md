# Provenance

Open CASCADE is used solely as the differential test oracle: driven through
its Python bindings (`cadquery-ocp`) in `tools/oracle/`, which carries its
own `pyproject.toml` and is never a build or runtime dependency of any Rust
crate in this workspace — not even a dev-dependency.

Its source is read, never copied: algorithms and known failure modes are
studied there and reimplemented from scratch against Arris's own
representation, with no OCCT code, comments or identifiers carried over and
no line-by-line translation.
