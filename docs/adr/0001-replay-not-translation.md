# Replay, not translation

A native SolidWorks file cannot be written by third parties: `.sldprt` is
proprietary and undocumented, its feature tree has no public schema, and
SolidWorks itself imports competitors' native files only as bodies. We therefore
rebuild SolidCraft recipes inside SolidWorks through its public API (a generated
macro), accepting that the result is a *reconstructed* tree rather than a
translated one.

Considered: file translation (impossible at any budget); neutral-geometry export
only (loses the tree — still used *within* a rebuild as the fallback path).

Consequences: fidelity is defined per feature; anything unmappable degrades to a
fallback body with a report instead of failing the whole model, and marketing must
never claim "lossless conversion".
