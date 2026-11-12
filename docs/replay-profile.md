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
| `recipe.json` | The SolidCraft script JSON, verbatim: an object with a `commands` array of `{command, params}` entries. |
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

## Inspect

`craft-bridge inspect <artifact> [--json]` reads the artifact and reports it;
it executes nothing. Steps are the recipe's commands in order (unknown command
ids are listed as they are — preserved, never interpreted). Parameters keep
their raw JSON so expressions and units survive: user parameters come from
`parameters.add`, model parameters from inside `timeline.add`. The `--json`
shape is part of the contract and is pinned by golden tests.

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
