# Replay profile `craft-bridge/replay/1`

The replay profile is the versioned agreement on what a craftx artifact
contains and how a rebuild consumes it. The container shape follows
SolidCraft's draft RFC 0001 (`*.craftx`); this profile layers the replay
payload over it. Compatibility with a future CamCraft bridge is an intent,
not a claim of established interoperability.

## Members

Exactly three members, exact names, case-sensitive. Nothing else may appear:
no directories, no duplicates, no alternate spellings.

| Member | Content |
|---|---|
| `manifest.json` | The manifest (below). |
| `recipe.json` | The recipe JSON, verbatim. Two shapes are accepted (below): a command script (`commands` array) or a native `solidcraft/57` design (`features` + `params`). |
| `geometry/final.step` | One STEP body (ISO-10303-21 text) from the end of the recipe: the coarse fallback snapshot. |

`manifest.json` is excluded from the manifest's own inventory to avoid
self-size ambiguity. The inventory (`members`) lists the other two with their
exact byte sizes; inspect refuses an artifact whose members disagree.

## Manifest fields

| Field | Meaning |
|---|---|
| `profile` | `craft-bridge/replay/1`. Any other value is refused with a clear error. |
| `container` | `craftx/1`, the container shape this artifact follows. |
| `name` | The artifact's name (the output file stem unless `--name` says otherwise). |
| `created_by` | Tool and version that packed the artifact. |
| `source.recipe_file`, `source.step_file`, `source.warnings_file` | Input file names (names only, no paths). |
| `warnings` | The omission-warning capture (below). |
| `fallback_mode` | `final-snapshot-coarse` in this profile: one STEP body replacing everything from the first unmappable step onward. Per-boundary snapshots are a later profile capability (ticket #6). |
| `members` | Payload inventory: `[{path, bytes}]`, excluding `manifest.json`. |

## Omission warnings (the sidecar contract)

SolidCraft's timeline script export reports what it left out (appearances,
configurations, canvases, …) *outside* the written script: the engine returns
an `omitted` list and `solidcraft-cli script` prints it on stderr. Capture
those strings into a JSON file — an array of strings — and hand it to
`pack --warnings`. The manifest then records:

- `status: "provided"` with `items: []` — the exporter reported no omissions;
- `status: "provided"` with `items: [...]` — the omissions, verbatim;
- `status: "not-provided"` — no sidecar was given, so the exporter's omissions
  are **unknown**. Pack and inspect both say so prominently; nothing claims
  that all warnings were preserved.

## Recipe shapes

Two shapes validate, plan and inspect through the same typed replay
planning. The 57 feature payloads (`sketch`, `extrude`, `box`, `cylinder`)
are shape-identical to the script's `feature` object, so both shapes feed
the same parsers - this is a reader seam, not a format translation.

### Command script (legacy, no `format` field)

An object with a `commands` array of `{command, params}` entries. Feature
payloads ride inside `timeline.add` steps under `params.feature`; design
parameters come from `parameters.add`. Recipes without a `format` field are
read as this shape, unchanged.

### Native design (`format: "solidcraft/57"`)

`params` and `features` at the root. Every feature has a unique integer
`id` (source identity, below) and a `type`. `units` is the design length
unit (mm, cm, m, in, ft). A `marker` is a timeline rollback cursor (a
zero-based index into `features`, source-pinned): the feature at that
index and every feature after it are rolled back - replay records them as
`not-executed` (named) and never builds them, because the reference
geometry excludes them. A malformed marker refuses.

**Source identity, dense runtime indices, snapshot addressing** are three
different things:

- *Source identity* is the feature's `id` (plus name/type in the reports).
  It travels in `PlannedStep.source_id` and in sketch references. Sparse,
  huge (below 2^52) and out-of-order ids resolve by map lookup.
- *Runtime indices* are dense `0..n` over the planning order: parameter
  equations first (the features reference them), then the features in file
  order. They bound macro arrays and report ordering.
- *Snapshot boundaries* address runtime indices, never source ids.

**Parameters** (`{name, expr, unit, comment?, model?}`) become global-
variable equations. `model` is provenance only (a dimension/feature-created
parameter like `d1`); the evaluator treats both kinds the same, and so does
replay.

**Suppression.** `suppressed: true` keeps the record and skips the
geometry, exactly as the source timeline does. The step plans as
`not-executed` (named) and never triggers the stop rule.

**Curves and constraints.** Curve indices refer to `curves`, point
indices to `points` (never the string ids). `construction`, `centerline`
and `infinite` curves all sit outside profile detection and replay as
construction geometry. An `angle` constraint with `flip: true` selects
the supplementary branch and degrades named (never a wrong angle). The
`fix {p}` constraint is a point pin and replays as a real `sgFix`
relation on the point's endpoint (the sketch origin is fixed by
construction; a free non-origin point has no entity to pin and degrades
named). A constraint referencing a point no curve endpoint occupies
degrades named - valid source data never refuses the build.

**Coordinate drivers.** A sketch point may carry `exprs: {x?, y?}`. The
expression is the source of truth (the source engine fixes expression
points at their evaluated positions before solve; `pos` is a cache): each
expressed coordinate becomes a native driver - a DistanceX/DistanceY
dimension bound by an equation to the expression - so parameter edits
carry the sketch geometry. Precedence: the expression drives even when
the point is `fixed` (the dimension both pins and parametrizes it). The
drivers measure DELTAS from one pinned anchor: a curve endpoint the
source pins (`fix` or `fixed` with no moving expression) that is held by
a real `sgFix` relation, at the sketch origin when possible. The anchor's
fixed offset folds into the equation. No pinned anchor degrades the
drivers named. A zero delta cannot carry a zero-length dimension: it is
position-verified and recorded as a driver note. An
expression outside the supported subset degrades named. A `fix` constraint
is a point pin and a faithful no-op (points are placed at their evaluated
positions already).

**Operations.** Only `operation: "new_body"` replays today: the emitted
`FeatureExtrusion2` merge/scope flags are hardcoded and proven for
new-body creation only. `join`, `cut` and `intersect` carry target-body
semantics that wait for the body registry and the merge/scope probe
(Phase B) - they stop named rather than operating on whichever bodies
SolidWorks happens to select.

**Named stops and prefix semantics.** Every unmapped feature stops with
its source id, name, type and the specific reason (the rollout phase named
where one carries it). Everything after the first stop is `not-executed`
unless a boundary snapshot covers the stop: the replayed result is a
supported PREFIX, and the reports say so - never "every later supported
feature ran".

## Inspect

`craft-bridge inspect <artifact> [--json]` reads the artifact and reports
it; it executes nothing. Steps are the recipe's runtime steps in planning
order (unknown command ids and unmapped feature types are listed as they
are - preserved, never interpreted). Parameters keep their raw JSON so
expressions and units survive: for a script, user parameters come from
`parameters.add` and model parameters from inside `timeline.add`; for a
57 design, `model: false` parameters are user parameters and `model: true`
are model parameters. The `--json` shape is part of the contract and is
pinned by golden tests.

## Limits (hostile input)

Artifacts and inputs are untrusted. Reads are bounded during the read, sizes
use checked arithmetic, and the ZIP central directory is bounded first by the
archive size and then by the member count, before any member is touched.

| Limit | Default |
|---|---|
| Archive file size | 512 MiB |
| Members listed | 16 |
| One decompressed member | 256 MiB |
| Total decompressed per artifact | 512 MiB |
| Recipe JSON | 64 MiB |
| STEP geometry | 256 MiB |
| Warnings sidecar | 4 MiB |
| Recipe commands | 100 000 |
| Warnings entries | 10 000 |
| JSON nesting | serde_json's own recursion limit (128); never raised |
| Fallback snapshots per artifact | 32 |

## Output policy

Packing publishes the artifact atomically: staged exclusively in the
destination directory, then renamed into place. Symbolic-link destinations are
always refused; an existing file is refused unless `--overwrite` is given;
failures remove the staging file and leave any existing destination unchanged.

## Fallback snapshots

Beyond the three core members an artifact may carry snapshots:
`snapshots/step_NNNN.step`, the STEP body state after recipe step N has run
in SolidCraft. Each manifest entry records `step_index`, `member`, `bytes`
and `scope`: `boundary` (the state after that step; the fallback transaction
imports it and replay continues) or `final` (one end-of-recipe body; the
coarse rule — the first fallback imports it and replay terminates there).
At most 32 snapshots, at most one `final`, names exact, inventory verified.
Artifacts without snapshots keep the original coarse meaning and say so in
the plan report. Sidecar files published beside the macro at build time are
named `fallback-step-NNNN.step` and obey the same safe-output policy as the
macro itself, which is always published last.

## Deviations from draft RFC 0001

- The recipe payload is defined here; the draft RFC defines no recipe member.
- Face and parameter reference tables are not carried yet; they arrive with
  later tickets and later profile versions.
- No content-addressed blobs, source identity, revisions or capability
  strings yet; `manifest.json` is a plain member, not a content-addressed one.
- `geometry/final.step` is the always-present coarse fallback; the optional
  `snapshots/` payload is this profile's extension (see above), a step toward
  the draft's required-parts machinery.
