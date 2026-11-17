# Research brief 06 - SolidCraft command shapes and container/plugin data

Companion to briefs 01-02 (native `solidcraft/57` schema research). Those
reports nailed the SERIALIZED document format. This brief asks about the
other layer - the live command API our design agent uses to BUILD
fixtures - plus the container/plugin/config data we have not covered.

Why it matters: the same concept serializes differently than it is
commanded (we observed `solid.sweep` taking `operation: "new"` while the
serialized enum writes `new_body`; `sketch.constraint.fix` takes
`entities: ["l1.start"]` while the serialized constraint is
`{type: "fix", p: 1}`). A differential reference prevents fixture and
prompt errors at the source.

## Ground rules (same as brief 02)

- Source: the SolidCraft repository at the same release tag (v0.43.0) -
  `crates/engine/src/cmd/*` and `crates/engine/src/schema/*` are the
  command layer - plus official docs at docs.solidcraft.io. Cite URLs.
- Mark facts **source-confirmed** / **documented** / **unverified**
  (runtime). Paraphrase prose; code identifiers and signatures are fine.
- Deliverable: one markdown report; the differential tables first.

## T1. Command-vs-serialized differential cheat sheet

For each command the fixture agent uses, a two-column table: command
argument names/forms vs the serialized feature fields they become.
Minimum set: `parameters.add`, `text_param.add` (if it exists),
`sketch.create`, `sketch.line`, `sketch.circle`, `sketch.arc`,
`sketch.conic`, `sketch.spline` (+ handles), `sketch.ellipse`,
`sketch.constraint.fix/coincident/horizontal/vertical/...`,
`sketch.dimension` (length/distance_x/distance_y/radius/angle), point
creation with expressions (`sketch.point`), `sketch.finish`,
`solid.extrude`, `solid.revolve`, `solid.box`, `solid.cylinder`,
`sphere/torus/cone` (names?), `solid.sweep`, `solid.loft`, `solid.fillet`,
`solid.chamfer`, `solid.combine`, `solid.move`, `solid.remove`,
`plane.create`/`construction_plane` (name?), `pattern`, `mirror`,
`suppress`/`unsuppress`, `timeline.inspect`, `set_parameter`,
`check_parameters`, `measure`, `inspect.compare`, `save`, `export`.
For each: which arguments are REQUIRED, accepted value forms (expression
strings? numbers in mm? names vs ids?), and the mapping to serialized
fields (including renames like `body_name` -> `body_names: [x]`).

Also: which commands have typed schemas under `--param-check strict`
(`describe_command` availability), and which are legacy/untyped.

## T2. Sketch-point and constraint command shapes

- The exact `sketch.point` argument forms: numeric pairs vs expression
  strings vs mixed; how `fixed` interacts with expression points in the
  COMMAND layer (we know the serialized result: `fixed: true` + `exprs`).
- `sketch.constraint.*` entity-name grammar: `l1.start`/`l1.end`/`c1.c`?
  how constraints name their entities vs the serialized index forms
  (`p`, `q`, `l`, `a`, `b`, `c`).
- Dimension commands: which produce `param_names` slots (dN) and how the
  slot ORDER is assigned per feature kind (which serialized input fields
  pair with `param_names[i]` in order - open question from brief 02).

## T3. Booleans, bodies, moves via commands

- `solid.extrude`/`solid.box`/... `operation` argument values at the
  command layer (`"new"` vs `"new_body"` etc. - the full alias table the
  parser accepts).
- Body naming (`body_name`), targeting (`targets`?), `keep_tools`
  arguments per command; what `solid.combine` accepts and returns.
- `solid.move` command argument names (brief 02 A4 covered the serialized
  fields; the command form may differ).

## T4. Container, plugins, configs (the uncovered schema)

- The `.sc` container `plugins/` records: what plugin-owned data can a
  design carry (shape of the records), and can ANY of it affect evaluated
  geometry - or is it inert metadata? What would a replay tool need to
  refuse or preserve?
- `configs` (configurations): the serialized shape; what a config row can
  override (parameters? suppression? feature values? sketches?); are
  configs part of the evaluated model or a separate state?
- `text_params` and sketch text expressions: how text-geometry features
  serialize (brief 02 A6 covered the parameter shape; this is the
  geometry side).
- `metadata`, `notes`, `favorites`, `browser_groups`, `timeline_groups`,
  `canvases`, `point_clouds`: one line each - can any affect geometry or
  evaluation, or are they purely presentational? A replay tool needs a
  complete "safe to ignore" list.
- `FaceAnchor`/`FaceCertificate` (brief 02 A3 left the certificate
  variants unenumerated) and `DatumRef` browser-key strings.

## T5. Export and verification surface

- `export` STEP options (AP242? tessellation? which bodies?) and what
  the exporter guarantees about body names/colors in STEP.
- `measure`/`inspect.compare` output field meanings (we consume
  volume_mm3/bbox/center_of_mass; confirm units and closure semantics,
  and what `open_bodies_skipped` means).
- `check_parameters` restore semantics (guaranteed restore on failure?)
  and what `recomputed` counts.

## Unknowns table

Same shape as the other briefs: question / status / where to look (source
paths) / why the replay cares.
