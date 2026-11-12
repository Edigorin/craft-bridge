# Plain-text .swb macro as the v1 artifact

The v1 bridge artifact is a plain-text `.swb` (SWBasic) macro the user runs in
SolidWorks, not an add-in or a COM runner. `.swp` is binary with no supported
programmatic authoring path, while `.swb` is the documented run-from-text route
that SolidWorks converts to `.swp` on run (its own Task Scheduler macro templates
are text `.swb` files; the API community documents it as the way third-party
tools create macros on the fly). Text is reviewable, portable, and the
lowest-exposure artifact to distribute, since commercially distributing
API-connecting software may require Dassault's Solution Partner program — the
exact EULA position is **unverified and remains an open legal question**; this
choice minimises exposure, it is not a legal opinion.

Considered: standalone C# COM runner (best harness ergonomics and the likely
future add-in core, but API-connecting distributed software); VSTA C# macro
(needs a Visual Studio install per seat).

Consequences: single-module macros (no forms or classes); the seat-side harness
connects via COM on the operator's own seat only; "SolidWorks 2018 and later" is
a compatibility target until seat-side evidence exists — tested versions are
reported separately and the generator targets conservative API members.
