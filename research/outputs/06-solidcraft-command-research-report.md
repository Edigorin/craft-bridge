# SolidCraft v0.43.0: command API versus serialized design data

**Filed:** 2026-11-17, produced by the owner's research agent from the
v0.43.0 source. Answer to `research/inputs/06-solidcraft-command-shapes-and-plugins.md`.
What this report already changed in the codebase: `validate_57` refuses
documents with non-empty `configs` or `point_clouds` (both can change the
evaluated model - configs override parameters/suppression and rules
evaluate at runtime); sketches carrying `links` (text-generated outlines)
degrade named (the outline replays as frozen geometry, its text
parametrics are not replayed); the `dN` slot-order question is resolved
(the `FeatureKind::inputs()` orders below), and the unit rule (feature
bare numbers = design length unit; 2D point coordinates = millimetres)
is recorded in docs/replay-profile.md. Plugin data is confirmed inert for
built-in geometry - no refusal needed.

---

**Research date:** 2026-11-17
**Scope:** release tag `v0.43.0`; commands used by a design-fixture/replay agent, native `.sc` data, plugin and configuration records, STEP output, and verification.
**Evidence labels:** **S** = source-confirmed at the tag; **D** = stated in the repository's documentation at the tag; **U** = requires verification on the licensed/running application. No live SolidCraft commands or exports were executed during this research.

The source links in square brackets point to files at the same release tag. Each row below has the command implementation and the corresponding serializer/type source. The tables describe **input command objects**, not ready-made serialized feature objects. A parameter described as *required* is a required logical input; schema alternatives and defaults may make an individual JSON field optional.

## T1. Command-to-serialized differential cheat sheet

### A. Parameters, sketches, points and curves

| Command: required inputs; accepted forms and options | Serialized form or effect |
|---|---|
| **S** `parameters.add`: **name**, **expression** or **value**; JSON number or expression string; optional `unit`, `comment`, `favorite`. `unit: "text"` creates text rather than a numeric parameter. [P] [PS] | Numeric: `document.params[]` with `name`, `expr`, `unit` and comments. Text: `document.text_params[]` with `name`, `text`, `comment`. Favorite: `document.favorites`. No feature is created. [DOC] [TXT] |
| **S** `parameters.change`: **name** plus **expression** (or batch `changes: {name: expression}`); can add/rename/change parameters, `new_name?`, `unit?`. MCP alias `set_parameter` accepts `name` and `value` or `expression`. [P] [MCP] | Changes stored expressions or text; updates evaluated features. Its `recomputed` result counts features rebuilt, while `restored` counts cached results reused. `set_parameter` is **not** a distinct engine command. [P] [PARAM] |
| **S** `text_param.add`: **no command with this ID** at the tag. Use `parameters.add` with `unit: "text"`. [P] [TXT] | Writes `text_params[]`; sketch text may reference the named value through `text_expression`. [TXT] [LINK] |
| **S** `sketch.create`: **plane**: `XY`, `XZ`, `YZ`, construction plane name/id, explicit `{origin, x_dir, y_dir}`, or body-face reference `{face:[x,y,z],body?,index?,path?,fixed?}`. Optional `offset`, `flip` and names. [SK] | `features[]` entry of `type:"sketch"`, containing the resolved `PlaneRef` and `sketch:{points,curves,constraints,...}`; when attached to a face, `PlaneRef::Face` may contain a durable `FaceAnchor`. [DOC] [ANCHOR] |
| **S** `sketch.line`: **points** list, each `[x,y]`, point ID such as `"l1.end"`, or a snap object; optional `closed`, `construction`, `ids`, `infer`, `connect`. Coordinates may contain numeric literals or expression strings. [SK] | Sketch `points[]` entries plus `curves[]` items `{id,type:"line",a:<point_index>,b:<point_index>,construction?}`. Auto-inferred relations appear in `constraints[]`; strings are resolved to numeric indices. [SKMODEL] |
| **S** `sketch.circle.center`: **center**, **radius** or **diameter**; `through?`, `construction?`. Other IDs: `sketch.circle.two_point` and `.three_point`. **No bare `sketch.circle` command.** [SK] | Circle curve `{id,type:"circle",c:<point_index>,r:<number_mm>,construction?}`. Driven radius can generate a `radius` or `diameter` constraint with `param` and `value`. [SKMODEL] |
| **S** `sketch.arc.three_point`: **start, end, through** (coordinates or point refs). `sketch.arc.center_point`: **center, start** plus **end** or **sweep**; `clockwise?`. **No bare `sketch.arc`.** [SK] | Arc curve `{id,type:"arc",c,a,b,reversed?}`. `c`, `a`, `b` are point indices; `reversed` retains as-drawn orientation although stored arc geometry is anticlockwise. [SKMODEL] |
| **S** `sketch.conic`: **start, end, apex**; `rho?` in 0..1, default 0.5. [SKCREATE] | Curve `{id,type:"conic",a,b,apex,rho}`, with three point indices and a scalar `rho`. [SKMODEL] |
| **S** `sketch.spline.fit_point`: **points** 2..500, each point coordinate or ID; `periodic?`. [SKCREATE] | `{id,type:"spline",pts:[indices],control:false,degree:3,periodic?,handles?:[indices]}`. Fields at their defaults can be omitted on save. [SKMODEL] |
| **S** `sketch.spline.control_point`: **points** 2..500, cubic; `sketch.spline.control_point_5`: degree five. [SKCREATE] | Same `type:"spline"`, `control:true`, `degree:3` or `5`, `pts:[indices]`. Explicit NURBS data may appear for imported or referenced curves. [SKMODEL] |
| **S** `sketch.spline.handles`: **curve** (fit-spline ID); `reset_handles`: **curve**. `insert_point`: **curve,at**, optional `end`; `remove_point`: **curve,point**. [SKCREATE] | Updates the spline's `handles:[point_indices]`, point list, generated construction lines and constraints. The command output contains new handle/point IDs, not the serialized index array. [SKMODEL] |
| **S** `sketch.ellipse`: **center,major**, plus **minor** point or **minor_radius** expression; `axes?` default true. [SKCREATE] | Ellipse `{id,type:"ellipse",c:<index>,m:<index>,r:<minor_radius_mm>}`. With axes, additional construction lines and constraints are stored; the ellipse does not serialize those as inline coordinates. [SKMODEL] |
| **S** `sketch.point`: **point** as `[x,y]`, `"mid:<line>"`, `"on:<curve>:x,y"`, `"x:<a>+<b>:x,y"`, `{vertex:[x,y,z]}` or `{on_edge:[x,y,z]}`; `id?`. Each coordinate accepts a number or an expression. [SK] | Point `{id,pos:[x,y],fixed?,exprs?:{x?,y?}}`. A coordinate expression that **references a parameter** stores its expression; the point is fixed and follows parameter changes. An ordinary numeric coordinate is not automatically fixed. Snaps can add links or constraints. [SKMODEL] |
| **S** `sketch.dimension`: **entities** list of point/line/curve IDs (also projected model `{edge:...,body?}` or `{vertex:...,body?}`); `type?` = `auto`, `distance`, `horizontal`, `distance_x`, `vertical`, `distance_y`, `length`, `radius`, `diameter`, `major`, `minor`, `angle`; `value?` number, expression, or `name = expression`; `text_at?`; `driven?`. [SK] | A `constraints[]` item with `type` such as `distance`, `distance_x`, `distance_y`, `length`, `radius`, `diameter`, `angle`, `line_angle`, or `arc_sweep` and indexed operands, numeric `value`, `param?`, `driven?`, `text?`. `name = value` also adds a user parameter. [SKMODEL] |
| **S** `sketch.finish`: optional `restore_marker` (default true). [SK] | Commits the edited sketch on its feature. It does **not** serialize a separate `finish` feature. The temporary timeline marker is restored or transferred according to the option. [DOC] [SK] |
| **S** `sketch.text`: **text** or **text_expression**, plus baseline `at`; `height?`, `angle?`, `font?`, formatting and path style. `sketch.edit_text`: **link** or **entity** plus modified fields. [SKCREATE] | `sketch.links[]` has a link with `source:{type:"text",text,at,height,angle,font?,style?,exprs?}`. Generated text outlines are linked points and curves. `exprs` can contain `x,y,height,angle,text`. Text is **geometry-producing**, not merely a note. [LINK] |

**Unit distinction, S:** feature lengths passed as bare numbers use the **active design length unit**, whereas raw 2D point coordinate numbers in `sketch.point` and `sketch.line` are interpreted as **millimetres**. An expression string containing a user-parameter reference retains that dependency. Use explicit units in stored geometric expressions where possible. [TYPES] [SK]

### B. Constraint calls: names at the API, indices on disk

| Command: required inputs and accepted forms | Serialized constraint or effect |
|---|---|
| **S** `sketch.constraint.fix`: `entity:<point/curve ID>` or `entities:[IDs]`; `fixed?` boolean, otherwise toggle. [SK] | A point receives or loses `ConstraintKind::Fix{p:<point_index>}`; a curve is fixed by its **`Curve.fixed`** flag. Unfixing an expression-positioned point clears `SPoint.exprs` and leaves its current coordinates. [SKMODEL] |
| **S** `sketch.constraint.coincident`: **a,b** (point refs, point-on-curve, two lines, circle/arc pair, axes where accepted). [SK] | May emit `{type:"coincident",p,q}`, `{type:"point_on_curve",p,c}`, or several constraints (e.g. concentric/equal for circles), depending on operand types. [SKMODEL] |
| **S** `sketch.constraint.horizontal_vertical`: **line** ID or **points:[a,b]**; `mode?` horizontal or vertical. **No separate `.horizontal` / `.vertical` commands.** [SK] | `{type:"horizontal",l}` or `vertical` with curve index; for two points: `{type:"horizontal_points",p,q}` or `vertical_points`. [SKMODEL] |
| **S** `sketch.constraint.parallel`: **a,b** line or axis refs. `perpendicular`: **a,b**. `collinear`: **a,b**. [SK] | `{type:"parallel",a,b}`, `{type:"perpendicular",a,b}`, or `{type:"collinear",a,b}`; `a,b` here are curve indices. [SKMODEL] |
| **S** `sketch.constraint.tangent`: **a,b** (curves, axes, or shared endpoints as supported). [SK] | `{type:"tangent",a,b}` plus any coincident or point-on-curve constraints required by an endpoint selection. [SKMODEL] |
| **S** `sketch.constraint.equal`: **a,b** or `curves:[2..200 IDs]`. [SK] | One or more `{type:"equal",a,b}` constraints; a multi-curve call ties each to the first. [SKMODEL] |
| **S** `sketch.constraint.midpoint`: **point,line** or **a,b** in pick order. [SK] | `{type:"midpoint",p,l}` with point and curve indices; two-line selection may generate another point/constraint. [SKMODEL] |
| **S** `sketch.constraint.concentric`: **a,b** circles/arcs or one curve and a point. [SK] | `{type:"concentric",a,b}` for curves, or point-centre coincidence when one operand is a point. [SKMODEL] |
| **S** `sketch.constraint.symmetry`: **a,b** points or curves, plus **line** axis or **point** centre. [SK] | For point symmetry about a line, `{type:"symmetric",p,q,l}`; centre-point or curve symmetry can add generated construction geometry and additional constraints. [SKMODEL] |

### C. Solid operations and construction geometry

| Command: required inputs and accepted forms | Serialized `FeatureKind`, renamed fields and side effects |
|---|---|
| **S** `solid.extrude`: **distance** expression or `through_all:true`; `sketch?`, `profiles?` (all, indices, curve-ID loops or pick points), `face?` / `faces?`, `taper?`, `corner?`, `distance2?`, `direction?`, `to?`, `operation?`, `touching?`, `targets?`, `name?`, `body_name?`. [F] | `type:"extrude"`; `sketch`, `profiles`, nested `extent:{distance,direction,through_all,...}`, `operation:"new_body"` etc., `targets`, optional thin-wall settings. Feature wrapper `body_names:[...]`, `participants`, `param_names`. The command's `distance` is **not** a top-level serialized field. [DOC] |
| **S** `solid.revolve`: **axis** (sketch line ID, `x/y`, world axis, construction axis, referenced edge/face); `sketch?`, `profiles?`, `angle?` (default 360 degrees), `angle2?`, `direction?`, `to?`, `operation?`, `targets?`, `body_name?`. [F] | `type:"revolve"`; `axis:AxisRef`, `sketch`, `profiles`, `angle`, `angle2?`, `operation`, `targets`, angular convention. Axis strings are resolved to typed references, not necessarily copied literally. [DOC] |
| **S** `solid.sweep`: **sketch** profile, then `path_sketch` and **path:[curve IDs]** or `path_body` plus `path_edges`; `profiles?`, `orientation?`, `twist?`, `scale?`, `rails?` up to two, `operation?`, `targets?`. [F] | `type:"sweep"`; `sketch`, `profiles`, `path_sketch`, `path`, `path_body?`, `path_edges?`, `orientation`, `twist`, `scale`, `rails`, `rail_fit`, `operation`, `targets`. Command `operation:"new"` becomes serialized `"new_body"`. [DOC] |
| **S** `solid.loft`: **sections:[{sketch,profiles?} or {sketch,point}]** (2..50); `rails?` up to two, `rail_fit?`, Boolean/common fields. **Legacy/untyped.** [F] | `type:"loft"`; `sections`, `rails`, `rail_fit`, `operation`, `targets`. Runtime schema shape and corner cases need a probe because the command has no typed contract. [DOC] |
| **S** `solid.box`: **length,width,height** expressions; optional `corner:[x,y,z]` or `center:[x,y,z]`, Boolean/common fields. [F] | `type:"box"`; `corner`, `centred:bool`, `length,width,height`, `operation`. `center` is not a serialized `center` field; body naming goes to wrapper `body_names`. [DOC] |
| **S** `solid.cylinder`: **radius** or **diameter**, **height**; `base?`, `axis?`, `references?`, Boolean/common fields. [F] | `type:"cylinder"`; `base`, `axis`, `radius`, `height`, `references`, `operation`. Diameter is normalized to radius. [DOC] |
| **S** `solid.sphere`: **radius** or **diameter**; `center?` and Boolean/common fields. [F] | `type:"sphere"`; `center`, `radius`, `operation`. No `diameter` field. [DOC] |
| **S** `solid.torus`: **major,minor** radius expressions, `center?`, Boolean/common fields. [F] | `type:"torus"`; `center`, `major`, `minor`, `operation`. [DOC] |
| **S** `solid.cone`: **radius** or **diameter**, **height**; `top_radius?` or `top_diameter?`, `base?`, `axis?`, Boolean/common fields. [F] | `type:"cone"`; `base`, `axis`, `radius`, `top_radius`, `height`, `operation`. Top diameter normalized to top radius. [DOC] |
| **S** `solid.fillet`: **edges** coordinate picks or `{body,index}`, **radius**; `body?`, `tangent_chain?` default true, `type?` constant/curvature/chord/variable/spline, relevant style dimensions. [F] | `type:"fillet"`; `edges` as geometric picks, `radius`, style data, with persistent `edge_refs` and `edge_names` on the enclosing `Feature`. Not simply an array of command-edge IDs. [DOC] |
| **S** `solid.chamfer`: **edges**, **distance**; optional `distance2` or `angle`, `flip?`, `body?`, `tangent_chain?`. [F] | `type:"chamfer"`; `edges`, `distance`, `distance2?`, `angle?`, `flip?`, `body?`; persistent pick names and refs on enclosing feature. [DOC] |
| **S** `solid.combine`: **target** body, **tools:[body names]**; `operation?` join/cut/intersect, `touching?`, `keep_tools?`, `new_component?`, `component_name?`. [F] | `type:"combine"`; `target`, `tools`, `operation`, `keep_tools`; placement/ownership and naming can also change. `new_component` is not necessarily an inline `Combine` field. [DOC] |
| **S** `solid.pattern.rectangular`: one of **features**, **bodies** or **components**; **dir1,count1,spacing1**; `dir2?`, `count2?`, `spacing2?`, alignment/reversal flags. **Legacy/untyped.** [F] | `type:"pattern"`; selected feature/body references and `pattern` variant with rectangular dimensions. Component patterns can involve occurrence data. [DOC] |
| **S** `solid.pattern.circular`: one of **features**, **bodies**, **components**; **axis,count**; `angle?` default 360, `alignment?`, `reversed?`. [F] | `type:"pattern"`; chosen entities and a circular `pattern` variant with axis, count, angle. [DOC] |
| **S** `solid.pattern.path`: **features** or **bodies**, **path_sketch,path,count**, **spacing** or **distance**; `orientation?`, `flip?`. **Legacy/untyped.** [F] | `type:"pattern"` with a path-pattern variant. Paths reference sketches and curve IDs, not solely a geometric polyline. [DOC] |
| **S** `solid.mirror`: **features** or **bodies**, **plane** (`XY`/`XZ`/`YZ`, named plane, or explicit origin and directions), `combine?`. **Legacy/untyped.** [F] | `type:"mirror"`; `features`, `bodies`, `plane:PlaneRef`, `combine`, and placement data as applicable. [DOC] |
| **S** `solid.move`: **bodies:[names]**; `translate?`, `axis?`, `angle?`, `origin?` or rigid `matrix?` (3x4/4x4, flat or rows); `tolerance?`, `copy?`, `occurrences?` (for component copies), `name?`, `body_name?`, `body_names?`. [F] | `type:"move"`; `bodies`, `translate:[Expr;3]`, `rotate_axis?` (command **axis** is renamed), `origin?`, `angle?`, `copy?`, `copy_frames?`. The matrix form is reduced to the persisted transform representation. `body_name`/`body_names` target wrapper `Feature.body_names`. [DOC] |
| **S** `solid.remove`: **bodies:[names]**. [FM] | `type:"remove"`; `bodies`, suppressing their existence later in the evaluated timeline. It is not a destructive edit to an earlier feature. [DOC] |
| **S** `construct.plane.offset`: **base** (origin/named plane or planar face), **offset** expression or **through** point, `name?`. **No `plane.create` command.** [F] | `type:"construction_plane"`, holding `plane:PlaneRef` with offset or through-point references. [DOC] |
| **S** `construct.plane.at_angle`: **base,axis,angle**; optional `direction_only`, `pivot`, `name`. [F] | `type:"construction_plane"`, `plane:PlaneRef` as an angle-derived plane with referenced base/axis. [DOC] |

### D. Timeline, inspection, save, export and MCP aliases

| Command / tool: required inputs and key options | Persisted effect or result |
|---|---|
| **S** `timeline.suppress`: at least one of **feature**, **features** (IDs or names), **groups** (timeline folder IDs); `suppressed?` boolean, otherwise toggle. **No independent `.unsuppress` command.** [EDIT] | Changes each `Feature.suppressed`. Group calls may also suppress joints. A suppressed feature is not evaluated. [DOC] |
| **S** `timeline.inspect`: **feature** ID/name or **group** ID. [EDIT] | Read-only output `{definition,inputs:[{label,param,expression,unit}],error,warning,...}` for feature, or group membership. `inputs` is the authoritative dynamic `param_names` mapping. [ES] |
| **S** `timeline.edit`: **feature**, **set** fields (including dotted paths). [EDIT] | Edits nested serialized feature definition without changing feature ID; refreshes the model. [ES] |
| **S** `parameters.change` / MCP `set_parameter`: **name** plus value/expression. [P] [MCP] | See table A; named feature input `dN` can also be edited as a parameter, depending on the field. [PARAM] |
| **S** `inspect.measure`: `bodies?` name list or `items?` selected geometry; `names_only?`. MCP `measure` also has `body?` and `summary?`. [INS] [IS] [MCPDOC] | Read-only metrics; does not serialize a feature. Returns per-body `closed`, `volume_mm3`, `area_mm2`, `center_of_mass`, `bbox`, topology counts, and totals. [IS] |
| **S** `inspect.compare`: optional `a,b` body names (or default selection), `tolerance?` 0.001..10 mm, `spacing?` 0.01..1000 mm, `max_points?` 10k..50M, `max_regions?`, `color?`. [COMP] | Read-only geometric comparison unless `color` is requested; result includes `difference`, `deviation`, `identical`, material-only regions, warnings and sampling data. It is a sampled comparison, not an exact proof of equal solids. [CS] |
| **S** `file.save`: `path?` if already named, else required; `format?` container/json, `compression?` fast/compact, `cache?`, `preview?`, `overwrite?`; `file.save_as` requires **path**. [FILE] [SAVE] | Writes an `.sc` ZIP container or JSON. `sign` is restricted to user-initiated Save; scripts/agents cannot sign. A normal save retains plugin data and known design state. [SAVE] [IO] |
| **S** `file.export_step`: **path**, `bodies?`. `file.export`: **path**, `format?` (e.g. `step`), `bodies?` and format-specific options. **No standalone engine command `export`.** [FILE] [FILES] | Writes AP242 STEP B-rep. Reply includes `{path,bytes,format}`. No AP version or mesh-quality option exists for `file.export_step`. Names, colours and the assembly hierarchy are passed to the STEP writer. [STEP] |
| **S** MCP `save` / `export`: tool shortcuts, not the same IDs as engine `file.save` / `file.export`. MCP `check_parameters` is also a tool, **not a command**. [MCPDOC] [MCP] | Operates by calling engine commands; `check_parameters` uses a checkpoint, changes parameters temporarily, measures, and restores. Its explicit `restored` result must be checked. [MCP] |

## T1 continuation. Contract coverage and discovery

**S — typed versus legacy commands.** In `v0.43.0`, the audited commands above have typed input/output contracts for `--param-check strict`, **except** `solid.loft`, `solid.mirror`, `solid.pattern.path` and `solid.pattern.rectangular`. The broader legacy/untyped exception list at this tag is:

```text
face.delete
render.aging.describe
render.backends
render.effects.describe
render.scene
solid.draft
solid.hole
solid.loft
solid.mirror
solid.offset_face
solid.pattern.path
solid.pattern.rectangular
solid.shell
solid.thread
surface.extrude
surface.revolve
```

This is the explicit list in [LEGACY]. For converted commands, `off` (default) bypasses the new contract; `warn` executes the handler and returns `param_warnings` if needed; `strict` refuses malformed inputs and unknown keys **before execution**. A legacy command's presence in the command registry does not imply that `strict` validates its complete input shape. Some command handlers themselves validate inputs. [SCH]

**S/D — introspection.** Use MCP `list_commands` to locate command IDs and `describe_command {"id":"solid.extrude"}` to obtain parameter descriptions and JSON Schema `input` / `output`. The engine equivalent is `engine.describe {"command":"solid.extrude"}`. For a legacy command, schema values are `null`. `list_commands` returns enablement based on the current editing state, so a supported command can be temporarily disabled. [MCPDOC] [IS]

**S — common typed forms.** A `LengthExpr` accepts a number (in current design units) or an expression string, such as `"W / 2"` or `"30 mm"`. An `AngleExpr` number means degrees. A 3D point expression can be `[x,y,z]` or `{x,y,z}`, each component numeric or an expression. A body-name list often accepts one name or an array of names. An existing feature reference may be a numeric ID or a name. These forms do **not** mean every field of every command accepts all forms: the exact contract still applies. [TYPES]

## T2. Sketch points, reference grammar, constraints and dimensions

### Exact entity grammar

| API reference | Resolution and saved representation |
|---|---|
| **S** `origin` | Sketch origin point, normally `points[0]`; the engine protects it from manual release. [SKMODEL] |
| **S** `p3`, `l1`, `c1`, `a1` | An existing point ID or curve ID. The ID prefix is a creation convention, not a serialized index. Look up `points[].id` or `curves[].id` before mapping. [SKMODEL] |
| **S** `l1.start` / `l1.end` | The indices in line/arc/spline ends, accounting for `Curve.reversed`. The engine resolves those at command time. [SKMODEL] |
| **S** `c1.center`, `a1.center`, `e1.center` | Centre point of circle, arc or ellipse. **`c1.c` is not the accepted documented alias.** [SKMODEL] [SD] |
| **S** `e1.major`, `k1.apex` | Major-axis endpoint of an ellipse; control apex of a conic. [SKMODEL] |
| **S** `x`, `y` | Sketch-axis references in commands that support axes. An axis may be materialized as a linked construction curve. [SK] [LINK] |
| **S** `mid:l1`, `on:c1:10,5`, `x:l1+l2:10,5` | Snap/selection grammar used by point creation or dimensions where supported. Do **not** store these strings as curve-point indices. [SK] |

**S — expression point behaviour.** `sketch.point {"point":["Width / 2","Height / 2"]}` stores a parameter-following point: `fixed:true` with `exprs` for its coordinates. A JSON number, for example `[5,10]`, is in millimetres; a string literal like `"5"` that names no parameter uses the design length unit and does **not** force a fixed point. Mixed inputs, such as `["Width / 2",5]`, are accepted. An unknown parameter or an invalid expression is refused by the handler. A later `sketch.constraint.fix {"entity":"p3","fixed":false}` releases an expression-driven point at its current numerical position, removing its `exprs`. [SK] [SKMODEL]

```json
{"command":"parameters.add","params":{"name":"Width","expression":"40 mm"}}
{"command":"sketch.create","params":{"plane":"XY"}}
{"command":"sketch.point","params":{"point":["Width / 2",5],"id":"datum"}}
{"command":"sketch.constraint.fix","params":{"entity":"datum","fixed":false}}
```

This is a **source-derived illustration, not an executed fixture**. Do not assume the chosen point ID is free in an existing sketch.

**S — operand mapping.** The command parser first resolves named refs to point and curve indices, then creates `ConstraintKind` values. A point-point distance uses `p,q`; a line length uses `l`; a radius uses `c`; a line angle uses `l` or `a,b` depending on the kind. A tangent or equal relation usually uses curve indices `a,b`. The serialized `Constraint` wraps the kind with its own ID and optional `param`, `driven` and text-placement fields. An operation can emit **more than one** constraint. The mapping is therefore not always 1:1. [SKMODEL] [SK]

**S — dimension details.** `sketch.dimension` accepts one or two entities, either as a single ID string or an array. Additional `type` values handled by the schema include `sweep`, `arc_length`, `overall_length` and `offset`. `distance_x` and `distance_y` are command aliases of horizontal/vertical dimensions. `orientation:"horizontal"` / `"vertical"` can disambiguate `auto` or `distance`. `driven:true` creates a measured reference dimension without driving the sketch. `if_redundant:"driven"` permits a reference dimension when the desired value is already fixed by other constraints. `scale_sketch` and `scale_about_origin` have special effects on an initial free-sketch dimension. See the typed contract before using them. [SD] [SK]

### Feature `param_names` slot order: the resolved rule

**S — exact algorithm.** For each feature, the document stores an ordered array `feature.param_names`. It aligns with `FeatureKind::inputs()`; `Document::all_param_exprs` zips these two sequences. `FeatureKind::inputs()` walks **input expressions only**. Coordinate expressions marked `placement` follow parameters but **do not receive `dN` names**. New input slots receive the next unused `dN` and existing names are preserved. Optional inputs enter the sequence only when present; editing a feature can change the number of slots. Sketch driving dimensions instead carry `constraints[].param`. [PARAM] [DOC]

| Serialized feature | Input-expression order at this tag (only present optional fields contribute) |
|---|---|
| `box` | `length`, `width`, `height`. The position `corner` is a placement expression, not a `dN` input. [DOC] [PARAM] |
| `cylinder` | `radius`, `height`. `base` is placement, `axis` is not an input expression. [DOC] [PARAM] |
| `sphere` | `radius`. `center` is placement. [DOC] [PARAM] |
| `torus` | `major`, `minor`. `center` is placement. [DOC] [PARAM] |
| `cone` | `radius`, `top_radius`, `height`. `base` is placement. [DOC] [PARAM] |
| `extrude` | Nested `extent` order: `distance`, `distance2?`, `start_offset?`, `taper?`, `to_offset?`; then optional `thin.thickness` and `thin.thickness2?`. The exact active sequence also depends on the feature's current form and walker conditions. [DOC] [PARAM] |
| `revolve` | `angle`, `angle2?`. [DOC] [PARAM] |
| `fillet` | `radius` plus expression inputs embedded in selected style variants where applicable. [DOC] [PARAM] |
| `chamfer` | `distance`, `distance2?`, `angle?`. [DOC] [PARAM] |
| `sweep` | `twist?`, `scale?`, plus any guide/rail expression inputs traversed by its variant. [DOC] [PARAM] |
| `move` | `translate[0]`, `translate[1]`, `translate[2]`, `angle?`. `origin` is placement; an explicit matrix is converted to these stored fields. [DOC] [PARAM] |
| `combine`, `remove` | No direct numeric input-expression slots in these variants. [DOC] [PARAM] |

**Replay rule:** Do not assign a `dN` number from a command's argument position. Inspect the resulting feature with `timeline.inspect {"feature":id}` and record `inputs:[{label,param,expression,unit}]`, or read the `param_names` array and the same-tag `FeatureKind::inputs()` walk. This avoids errors after optional inputs, new feature types and editing. **U:** The precise slot list for every possible rail, fillet variant, and pattern shape needs a generated exhaustive fixture at runtime; do not guess it from this table. [ES] [PARAM]

## T3. Boolean aliases, body identity, targeting and movement

### Command-operation aliases and serialized enum

**S:** The parser converts strings to an `Operation` enum after lowercasing and removing spaces, underscores and hyphens. Only these target canonical serialized values exist. [DOC] [TYPES]

| Command `operation` aliases accepted | Serialized JSON value |
|---|---|
| `new`, `newbody`, `newcomponent` (including separators and case variants) | `"new_body"` |
| `join`, `union`, `add` | `"join"` |
| `cut`, `subtract`, `difference` | `"cut"` |
| `intersect`, `intersection` | `"intersect"` |
| `auto` | **Extrude-specific command option**; the handler resolves it to an actual operation. Do not serialize an `Operation` enum as `auto`. |

**S — `solid.combine` is different.** It allows `join`, `cut`, `intersect` and their aliases; it does not accept `new`. `operation` defaults to `join`. `target` is one existing body, `tools` lists the contributing bodies, and `keep_tools:true` preserves the tools instead of consuming them. A geometric join does not guarantee that disconnected volumes become one closed component. The `touching` mode, where offered, can govern whether touching-only pieces stay separate or form one body. [F] [TYPES] [DOC]

**S — command versus file body names.** Creation commands accept `name` for the timeline feature, `body_name` for one output and often `body_names` for multiple outputs. The serialized `Feature` wrapper stores `name` and `body_names:[...]`; it does **not** have a `body_name` field. `participants` records affected existing bodies. `targets` is a separate `FeatureKind` field only for features that support Boolean targets. Empty `targets` often means the bodies eligible under the command and component's normal scope, not an empty Boolean operation. [TYPES] [DOC]

**S — body replay hazards:**

1. **Do not assume body counts from `operation`.** A cut may miss its target, a join may not touch, and touching-only contact can have separate-body semantics. Compare `created_bodies`, `changed_bodies`, `removed_bodies`, body count and measured volume after each operation. [MCPDOC] [DOC]
2. **Keep identities stable.** Persist explicit names for newly created bodies and read the engine's assigned `body_names` after evaluation. Removed body names can remain reserved, and renaming can affect later selectors. [DOC]
3. **Do not infer affected bodies from `body_name`.** The target of a cut is an existing body. It can have a different name from the body of the feature being created. Use the command's `targets`/`participants` rules and inspect its output. [F] [TYPES]
4. **Preserve component context.** A copied or moved body can have occurrence paths and component ownership state outside the `Move` feature. Replay those alongside the feature timeline if assembly equivalence is required. [DOC]

**S — `solid.move`.** The command may accept `translate`, `axis`, `angle`, `origin` or a rigid `matrix`. The serialized `FeatureKind::Move` has `translate`, `rotate_axis`, `origin`, `angle`, `copy`, and optional `copy_frames`. The command's `axis` becomes `rotate_axis`. The `matrix` form is parsed and stored as the model's transform components rather than a verbatim matrix key on `Move`. A copy retains source bodies and produces new ones. Check the resulting body names and measured positions, not just the status result. [F] [DOC]

## T4. `.sc` container, plugins, configurations and non-geometric data

### `.sc` container and `plugins/` data

**S.** The `.sc` format is a package with authoritative `document.json` and `container.json`. The writer also stores auxiliary assets and optional caches. Plugin-owned files are ZIP entries named `plugins/<plugin-id>/<path>`. They are **not** OS paths to extract and execute. In standalone design JSON, `document.plugin_data` is a map from `<plugin-id>/<path>` to a **base64** string. In a container, the writer removes that map from `document.json` and stores the decoded bytes as separate package parts. The reader converts them back to base64 in memory. [IO] [PLUGIN]

```json
{
  "plugin_data": {
    "tool-plugin/settings.json": "eyJ2ZXJzaW9uIjoxfQ==",
    "tool-plugin/data/example.bin": "AAEC"
  }
}
```

This shows **valid structural forms**, not a claim that these plugin IDs or records exist in a real design. The plugin decides its file contents. The package has no universal typed plugin-record schema. [PLUGIN]

**S — validation bounds.** A plugin key must be at most 255 bytes: an ID of 1–64 lowercase ASCII letters, digits and internal hyphens, followed by `/` and non-empty plain path segments. Segments may contain letters, digits, period, underscore and hyphen, but may not start with a period. `..` traversal is therefore rejected. Up to 1,024 plugin files are retained. Invalid keys or entries beyond the count limit are discarded during JSON deserialization. Package part reads are bounded. Preserve raw bytes and key names when reading and writing a file you own. [PLUGIN] [IO]

**S — geometry and security conclusion.** At this tag, the plugin-data feature is a future storage interface. The source states that nothing currently mutates these records through a plugin storage capability. The built-in geometry evaluator does **not** interpret data from `plugins/`. Therefore, plugin bytes are **safe to ignore for built-in geometry computation** but **not safe to delete in a lossless round trip**. A replay program should copy valid opaque data byte-for-byte, never execute or parse untrusted plugin scripts as geometry commands, and report rejected or unavailable parts. The state of future third-party plugins remains **U** for later releases. [PLUGIN] [IO]

### Configurations are evaluated state, not inert document metadata

**S — exact shape:** [CONFIG] [RULE]

```json
{
  "configs": {
    "columns": [
      {"param":"Width"},
      {"suppress":12},
      {"material":"Body1"}
    ],
    "rows": [
      {"id":21,"name":"Wide","cells":["60 mm","false","Aluminum"]}
    ],
    "row_identities": {},
    "active":"Wide",
    "rules": []
  }
}
```

The example illustrates the enum's externally tagged fields and ordered cells. It is **schema-derived, not a validated runtime save**. A valid configuration must refer to real parameter names, features and bodies and satisfy unit/rule checks. Limits at this tag: 500 columns, 2,000 rows. `active` is a **row name**; each row also has a numeric ID. Row-creation identities protect links from accidentally targeting a new row after undo. [CONFIG]

The `Column` enum has exactly six variants: `param(String)`, `suppress(u64)`, `material(String)`, `appearance(String)`, `component_appearance(u64)`, and `face_appearance(ConfigFaceTarget)`. A row has `id`, `name`, and `cells:[String]` with one cell per column. `config.activate` applies the row atomically. Parameter overrides and suppression can change evaluated geometry; materials change mass and appearance; component/face appearance changes visual state. A configuration is **not** a separate feature timeline or a frozen geometric body. [CONFIG] [CONFIGDOC]

**S — rule engine.** Each `rules[]` entry has `id`, `name`, `enabled?`, `length_unit?`, `condition`, `effect`. Conditions: `always`, `all`, `any`, `not`, parameter comparisons (`eq`,`ne`,`lt`,`le`,`gt`,`ge`), and feature suppression checks. Effects: `drive_parameter`, `drive_suppression`, `parameter_range`, `allow_parameter_values`. Ordered rules derive effective state **without overwriting saved row cells**. The limit is 2,048 rules; condition nesting is bounded. Replay must **evaluate** rules or delegate them to SolidCraft. Do not flatten them to a single saved row and discard the rules. [RULE]

### Classification of additional top-level document records

| Field / related record | Safe to ignore for evaluated solids? | Replay requirement and basis |
|---|---|---|
| `metadata` | **Yes, for geometry (S).** | Informational project/author/part/revision and third-party reverse-DNS keys. Retain for fidelity and downstream quoting/BOM. The metadata module explicitly excludes geometry evaluation. [META] |
| `notes` | **Yes for geometry (S).** | Screen-facing model-space annotations. Preserve if rebuilding review/UI state; do not turn them into sketch geometry. [DOC] |
| `favorites` | **Yes for geometry (S).** | Set of preferred parameter names; changes presentation/order, not expressions. [DOC] |
| `browser_groups`, `browser_order` | **Yes for B-rep computation (S).** | Browser folders and sort order. Names and grouping must remain if UI equivalence matters. [DOC] |
| `timeline_groups` | **Yes for the order of feature evaluation (S).** | Folder membership does not reorder features. However, commands can suppress entire folders, so preserve group membership for **future command replay**. [DOC] [EDIT] |
| `canvases` | **Yes for direct solid evaluation (S, limited).** | Reference images and their placements. Preserve if sketches were created with reference to them or UI/visual fidelity matters. The image itself is not a B-rep feature. [DOC] |
| `point_clouds` | **Not a solid by itself (S); dependency-dependent (U).** | Scanned reference points. Preserve unless the replay explicitly has no consumer of the cloud; derived features might depend on external references or user actions. [DOC] |
| `named_views`, `visibility` | **Yes for B-rep computation (S); not safe for complete agent replay.** | Cameras and hide/show state. Hidden bodies can affect selection and agent decisions even when the underlying B-rep is unchanged. [DOC] [MCPDOC] |
| `render`, `appearances`, `materials`, `user_materials` | **Usually yes for shape; no for visual/mass parity (S).** | Appearance changes STEP colours, rendering and physical material may change mass. Preserve where comparing exports or `inspect.measure.mass_g`. [DOC] [STEP] |
| `marker` | **No.** | A timeline rollback marker excludes features at or after its index from evaluation. [DOC] |
| `configs` | **No.** | Parameters and feature suppression change the effective model. [CONFIG] |
| `components`, `occurrences`, `assembly`, `links`, `parts`, `body_components`, `body_offsets` | **No.** | Affect placements, assembly hierarchy, joint-driven state and export geometry. Preserve where replay needs world coordinates or an equivalent STEP assembly. [DOC] |
| `text_params`, sketches' `links` with `source.type:"text"` | **No.** | Text expressions generate 2D outlines and can produce solid profiles. See next section. [TXT] [LINK] |
| `plugin_data` | **Yes, at this tag's native evaluator; preserve opaque bytes.** | Kept independently of plugin availability. The current plugin storage capability is inert. [PLUGIN] [IO] |

### Text geometry, FaceAnchor and DatumRef

**S — text parameters and sketch geometry.** `Document.text_params[]` contains `{name,text,comment?}`. `parameters.add {unit:"text"}` creates the record. `sketch.text` can take literal `text` or `text_expression` that combines named text parameters, numeric parameters via `number(...)`, and quoted literals. A sketch stores the text source under `sketch.links[].source` with `type:"text"`, `text`, baseline position `at`, `height`, `angle`, `font?`, `style?`, and `exprs?`. `TextExprs` has optional `x,y,height,angle,text`. Generated points and curves carry a `link` back to that text source. The document rebuilds the outline geometry as expressions change. A replay that only copies the visible outline loses the text's parametric meaning. [TXT] [LINK] [SKCREATE]

**S — exact face attachment.** `FaceAnchor` stores `target:ConfigFaceTarget`, `component`, `path`, `author_path`, optional `placement`, `coordinates:[f64;2]` and normalized `x_axis:[f64;2]`. `ConfigFaceTarget` stores `body`, `origin_feature`, persistent face `name`, a diagnostic `point`, and optional `certificate`. The resolver rejects lost identities instead of silently selecting a nearby face, except for the documented extrude-cap loop-change fallback. These fields affect a sketch attached to a generated face and are **not** optional display metadata. [ANCHOR] [CERTSTYLE]

**S — enumerated `FaceCertificate` variants**, serialized with tag `kind` in `snake_case`: [CERT]

```text
box          {axis:u8, high:bool}
cylinder_cap {high:bool}
extrude_cap  {sketch:u64, high:bool, outer:[curve IDs], holes:[[curve IDs]]}
extrude_side {sketch:u64, curve:string}
imported     {payload_sha256:string, item:u64, face:u64}
shell_planar {source_producer:u64, source:FaceCertificate, inner:bool}
lineage      (no extra fields)
```

`lineage` depends on the kernel's exact history name. A positional `point` is diagnostic, **not** a general fall-back face selector. A consumer must preserve the certificate and feature/body identities to replay an attached sketch. [CERT] [ANCHOR]

**S — `DatumRef` browser keys.** A datum of a specific assembly occurrence is serialized as a canonical string, e.g. `"o:2/3/origin/XY"`: the non-zero occurrence IDs from root to leaf, then `origin/` and one of `O, X, Y, Z, XY, XZ, YZ`. The parser requires decimal IDs without leading zeros, at most 64 path entries and 2,048 characters. `o:01/origin/X` and `o:0/origin/X` are invalid; root-only origin references use the ordinary origin-plane or geometry reference, not an empty `DatumRef` path. These datums **follow component placements during evaluation**. [DATUM]


## T5. Export and verification surface

### STEP export: accepted options and guarantees

**S — command contract.** `file.export_step` takes `path` (required) and `bodies` (optional body name or list). `file.export` can select STEP through `format:"step"` or a recognized file extension. When `bodies` is omitted, all bodies are eligible and designs with components are exported as assemblies. The `file.export_step` implementation selects STEP independently of the destination filename extension. **There is no STEP-specific AP203/AP214/AP242 selector, tessellation-quality setting or mesh chord-tolerance argument.** Those quality options belong to mesh exporters, not STEP. [FILE] [FILES]

**S — resulting data.** The active writer produces ISO 10303-21 **AP242** with a product structure. Each exported product has a name, its own bodies and child products with rigid placements. Each body has a requested name, its B-rep record, optional colour and optional per-face colour. A `StepHeader` also carries file name, timestamp, author, organization and description; empty description defaults to a generic SolidCraft description. The exporter explicitly refuses mesh-only bodies before writing STEP. [STEP] [STEPDOC]

**D/S — what "preserved" means.** SolidCraft writes its own names, colours and components into the STEP payload. This is a claim about **what SolidCraft emits**, not a guarantee that SolidWorks, FreeCAD or another importer will retain every name, colour, material or hierarchy unchanged. No such third-party round-trip was run for this report. **U:** verify required interoperability on the owner's target seat, especially multiple bodies in an assembly and coloured faces. [STEP] [STEPDOC]

**Replay policy:** Save the native `.sc` or JSON design as the authoritative editable source. Treat STEP as a derived interoperability artifact. After export, check that the reported output file exists, is non-empty and can be imported and measured independently. Do not use STEP face/entity numbering as a substitute for SolidCraft's persistent `FaceCertificate` or native sketch references. [STEP] [CERT]

### `inspect.measure`: units and closure

| Output field | Meaning at v0.43.0 | Replay check |
|---|---|---|
| `body_count`, `bodies[]` | Count and list of bodies selected for inspection. [IS] | Check both count and expected names, not only volume. |
| `bodies[].closed` | Whether a body's boundary encloses volume. [IS] | Require `true` for every body intended to be a solid. |
| `bodies[].volume_mm3` | Enclosed volume in mm³; absent or `null` for open bodies. [IS] | Do not treat an open body's missing volume as a measured zero. |
| `bodies[].area_mm2` | Surface area in mm², for closed and open bodies. [IS] | Useful for detecting unexpected surface-only outputs. |
| `bodies[].center_of_mass` | Geometric centroid at uniform density, as `[x,y,z]`; absent or `null` when volume cannot be measured. Coordinates are in the inspected body's reported frame (typically world in `inspect.measure`). [IS] | Check geometry placement separately from volume. |
| `bodies[].bbox` | Axis-aligned bounding box `{min:[x,y,z],max:[x,y,z]}` in mm. [IS] | Check size **and** position; a rotated part's AABB changes even when its volume does not. |
| `faces`, `edges`, `vertices`; `kernel` | Reported topology counts; the presentation count can merge pieces of one analytic face, while `kernel` contains lower-level counts. [IS] | Use the same metric on both reference and replay. |
| `measured_by`, `notes` | Source of measurement (`smlib` or `mesh`) and diagnostic notes. [IS] | Record measurement mode in test artifacts. |
| `total.open_bodies_skipped` | Number of open bodies excluded from aggregate volume and mass; they **are** included in aggregate area and topology counts. [IS] | Assert zero when the fixture expects only closed solids. |
| `total.volume_mm3`, `total.area_mm2`, `total.mass_g` | Aggregate measures. Volume and mass exclude open bodies; physical material assignment affects mass. [IS] | Do not compare mass unless material and density assignments are matched. |
| `bodies[].error` | Some bodies report an error instead of a complete measurement. [IS] | Fail the verification instead of inventing a numeric value. |

**D — compact MCP output.** MCP `measure {"summary":true}` omits expensive detail and keeps `name`, `volume_mm3`, `bbox`, `faces` and `closed` for each body, plus `body_count` and `total.volume_mm3`. Use the full command `inspect.measure` when surface area, mass, centre of mass, detailed topology or open-body totals are needed. [MCPDOC] [IS]

### `inspect.compare`: sampled difference, not a strict solid-equivalence oracle

**S — parameters:** give both `a` and `b` body names or neither (then it uses the eligible selection or the only two bodies). `tolerance` is a surface-agreement threshold in **mm**, from 0.001 to 10, default **0.01**. `spacing` controls sampling in **mm**, from 0.01 to 1,000, default **0.5**. `max_points` defaults to eight million per body and allows 10,000–50 million. `max_regions` defaults to 50 and allows 1–1,000. `color` can request coloured difference faces and changes appearance as one undoable action. [CS] [COMP]

**S — result fields:** `a` and `b` contain each body's measures. `difference` uses **b minus a** for volume, area, centroid coordinates and bounding-box coordinates. `volume_percent` is relative to `a`. `deviation.a_to_b` and `.b_to_a` describe two directed surface-sampling comparisons; each includes maximum/mean distance, location and area past tolerance. `deviation.max_mm` is the larger directed maximum. `regions`, `only_in_a`, `only_in_b` describe where only one body has material. `unlisted_regions` counts detected regions above the return cap; those are not fully measured. `warnings` explains incomplete or unreliable cases. `identical` means no region found above tolerance under the algorithm's tests; it is **not** formal mathematical identity or a guarantee against narrow unsampled defects. [CS]

**Replay policy:** For regression fixtures, combine signed position checks, `inspect.measure` volume/area/bbox/closure and `inspect.compare` under a chosen documented tolerance. Record sampling settings, warnings, body names and `unlisted_regions` with each result. If a comparison is thin-walled, nearly coincident or expected to differ below default spacing, decrease the spacing or add independent geometric probes. **U:** determine a useful sensitivity/false-negative envelope for the project's real parts on the target build; the schema alone cannot establish it. [CS] [MCPDOC]

### `check_parameters`: restore and `recomputed`

**S — MCP interface, not a persisted feature or engine command:**

```json
{
  "set": {"Width": "48 mm", "Height": 12},
  "report": ["measure", "cylinders", "errors"],
  "bodies": ["Body1"],
  "summary": true
}
```

The required `set` map must be non-empty; every value is a number or expression string. `report` defaults to all three listed categories. `bodies` is an optional **array of body names**; `summary` defaults to false. The tool checks known parameter names when a parameter listing is available. It refuses to try values if it cannot obtain a checkpoint. [MCP]

**S — restoration is attempted, not unconditional.** The tool first calls `edit.checkpoint`, applies test values, collects reports, then calls `edit.restore_checkpoint` **even if evaluation or reporting fails**. On successful restoration the reply includes `"restored":true`. If checkpoint restoration fails, the reply contains `"restored":false`, `restore_error` and an explicit warning that the temporary values **remain in the design**. A replay agent must test the `restored` field before any subsequent save or export. An error result with `restored:true` means the test failed but the design did go back. [MCP]

**S — two different meanings of `restored`.** In `parameters.change`, `recomputed` counts the features actually rebuilt, and `restored` counts earlier cached feature results reused by the evaluator. In **MCP `check_parameters`**, `restored` is a boolean for checkpoint restoration; its `recomputed` is the number of feature rebuilds during the trial (summed when changes must be applied sequentially). Do not confuse a cached result count with a restored editing state. These numbers do **not** count successful constraints, sketch dimensions, entities, or every possible downstream geometric effect. [P] [PARAM] [MCP]

**D — expected-geometry guard.** MCP `execute` or `batch` can take a tool-level `expect` object to verify total closed volume, body count, and bounding box after running commands. The documented default relative volume tolerance is 0.002 (0.2%), and each expected bounding-box coordinate is checked within 0.05 mm. An **expectation mismatch marks the tool result as an error but does not automatically undo the generated geometry**. A `batch` execution failure is different: with its default `rollback:true`, completed steps are rolled back when possible, and the response says whether rollback succeeded. Check the returned `expectation.ok` and `rolled_back` fields rather than assuming all errors preserve the starting model. [MCPDOC]

### Suggested verification sequence for craft-bridge replay

1. **Pin release and dialect.** Record tag, actual `engine.describe` schema, `--param-check` mode, native design units, and active component/configuration.
2. **Create each fixture deterministically.** Use explicit sketch and body names. Capture command results and assigned IDs; never assume implicit `lN`, `pN`, `dN`, or `BodyN` numbering is stable across unrelated fixtures.
3. **Measure at stages.** For each consequential Boolean or transform, record `closed`, body count/names, volume, bbox, centre of mass and any evaluation errors. Check that expected cuts and joins changed volume or topology.
4. **Save and reopen.** Compare document-native feature definitions, dynamic input parameter names, sketches, configurations and component placements; retain opaque plugin entries without interpreting or executing them.
5. **Export STEP and validate it independently.** Check B-rep closure, volume, bbox, assembly occurrence transforms, body names and colours on the toolchain actually used by the owner.
6. **Use temporary parameter sweeps carefully.** Require `restored:true` from `check_parameters`; stop instead of saving when it is false.

The sequence is a **recommendation derived from the confirmed API behavior**, not a claim that it has already been executed. [MCPDOC] [MCP] [IS] [STEP]

## Unknowns and probe queue

The statuses below refer to what this **static source review** establishes. A command may be implemented and documented but still require a runtime probe to confirm the agent-to-engine path, selected editing state or third-party output.

| Question | Status | Exact source or probe location | Why craft-bridge replay cares |
|---|---|---|---|
| Can the four legacy fixture commands accept the proposed JSON forms under `--param-check strict`? | **S:** no typed contract for `solid.loft`, `solid.mirror`, `solid.pattern.rectangular` or `solid.pattern.path`. **U:** exhaustive accepted shapes, defaults and silent aliases. | `crates/engine/src/cmd/features.rs`, `crates/engine/src/schema/unconverted.txt`; run `describe_command` and minimal `execute` fixtures. [F] [LEGACY] | A fixture may run with defaults or discard a misspelled argument without typed-schema rejection. |
| Does the running app expose the same v0.43.0 command registry and contracts? | **U:** target seat/bridge version and effective flags not tested. | `engine.commands`, `engine.describe`, MCP `list_commands`/`describe_command`, app release/build stamp. [MCPDOC] | Prevents mixing command shapes from one release with document serialization from another. |
| What is the full `dN` input-slot sequence for every optional loft/rail, fillet, pattern and other variant? | **S:** algorithm and common orders; **U:** exhaustive per-variant inventory. | `crates/doc/src/params.rs` (`FeatureKind::inputs`), `crates/doc/src/document.rs` walk attributes, feature fixtures with `timeline.inspect`. [PARAM] [DOC] | A wrong slot index changes the wrong geometric variable. |
| How are `auto` operations and empty target lists resolved in each component/body context? | **S:** aliases and `auto` scope; **U:** all ambiguity cases. | `crates/engine/src/cmd/features.rs`, `crates/engine/src/schema/types.rs`, runtime 0/1/multiple target probes. [F] [TYPES] | A join or cut can affect an unintended existing body. |
| How do `solid.combine` `keep_tools`, disconnected intersections and touching-only cases affect body identity? | **S:** input intent; **U:** full body count/name topology across cases. | `crates/engine/src/cmd/features.rs`, document `Touching` enum, Boolean tests, inspect output. [F] [DOC] | Separate solids might be incorrectly merged or deleted in an oracle fixture. |
| Does `solid.move` with `matrix`, `origin`, `copy`, or an occurrence path preserve expected world placement after reopen? | **S:** command-to-`Move` mapping; **U:** complete round-trip cases, especially copied assembly bodies. | `crates/engine/src/cmd/features.rs`, `crates/doc/src/document.rs`, `document.inspect` after save/reopen. [F] [DOC] | Coordinate frames and copies cause mismatched centre of mass or mating positions. |
| Do third-party CAD importers preserve the AP242 component tree, body names and body/face colours? | **S/D:** writer encodes them; **U:** downstream interoperability. | `crates/kernel/src/step_out.rs`, `docs/book/src/guide/files.md`; import the exported STEP into the owner's SOLIDWORKS seat. [STEP] [STEPDOC] | Avoid claiming round-trip identity based only on the exporter. |
| Which AP242 edition and optional STEP entities does the writer emit for each current feature? | **S:** AP242 protocol; **U:** detailed profile/edition matrix not established here. | `crates/kernel/src/step_out.rs`, `solidcore/crates/solidcore-step/src/write/product.rs`, parse resulting `FILE_SCHEMA` and representative entities. [STEP] | Importer compatibility and stable B-rep comparisons may depend on emitted entities. |
| Does the active exporter reject or transform an open surface, multi-shell B-rep or mesh body? | **S:** mesh body refused for STEP; **U:** open-surface/multi-shell behavior by feature and importer. | `crates/kernel/src/step_out.rs`, `crates/engine/src/cmd/file.rs`, trial exported files. [STEP] [FILE] | A valid file-write result is not proof of the desired watertight geometry. |
| Which face certificates remain stable after fillets, Boolean blends, imports or parameter edits? | **S:** certificate variants and exact identity checks; **U:** all topology-history transitions. | `crates/doc/src/config_certificates.rs`, `crates/doc/src/face_anchor.rs`, `docs/face-attachments.md`, face-attachment tests. [CERT] [ANCHOR] | A face-attached sketch must fail clearly, not silently attach to a different surface. |
| Does a particular stored `point_clouds` entry feed a live geometry feature or only reference display in our fixtures? | **U:** not fully audited beyond top-level structure. | `crates/doc/src/document.rs`, point-cloud command/schema modules and reference consumers; compare document evaluation with/without cloud on a copy. [DOC] | Determines when the replay can omit a large cloud without losing dependencies. |
| Can malformed, oversized or unknown plugin parts be preserved byte-for-byte through every load/save path? | **S:** bounded key/count handling, missing/invalid parts may be omitted; **U:** file-specific limits and warning behavior. | `crates/doc/src/plugin_data.rs`, `crates/io/src/design.rs` and round-trip tests. [PLUGIN] [IO] | Preservation of opaque plugin data is subject to validation and capacity limits. |
| Does any future installed plugin read opaque payloads and modify geometry? | **S:** v0.43.0 native plugin data inert; **U:** future releases and plugin behavior. | `crates/doc/src/plugin_data.rs`, plugin capability implementation at the actual deployed tag. [PLUGIN] | A "safe to ignore" decision is **version-bound**, not permanent. |
| Can `check_parameters` fail to restore a checkpoint in the owner's bridged application? | **S:** error path exposes `restored:false`; **U:** frequency and recovery on this runtime. | `crates/mcp/src/tools.rs`, `crates/mcp/src/tests.rs`; force a failed trial with checkpoint tests. [MCP] | Avoid accidentally saving a model containing temporary parameter values. |
| What numerical tolerance and sampling settings reliably distinguish small feature defects on the real fixture corpus? | **S:** argument ranges and report semantics; **U:** empirical sensitivity. | `crates/engine/src/schema/compare.rs`, `crates/engine/src/cmd/compare.rs`; controlled near-coincident comparisons. [CS] [COMP] | Default `identical:true` must not create a false "verified CAD equivalent" claim. |

**Decision for the fixture agent:** Use typed command schemas where available, build against the **live command API**, and validate the **serialized feature** independently. Preserve plugin bytes and all geometry-affecting configuration/reference fields. Avoid silently converting a reference or a sampled comparison into an exact proof. Where the unknowns table says **U**, put a minimal runtime probe in the queue before promoting the behavior to a replay invariant.

## Source index

The following paths refer to **SolidCraft v0.43.0**. The pinned repository source is the main authority; public reference pages may reflect a later version.

[P]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/parameters.rs
[PS]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/parameters.rs
[DOC]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs
[TXT]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/text_params.rs
[SK]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/sketch.rs
[SKCREATE]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/sketch_create.rs
[SKMODEL]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs
[LINK]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/link.rs
[ANCHOR]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/face_anchor.rs
[F]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/features.rs
[FM]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/features_more.rs
[EDIT]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/edit.rs
[ES]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/edit.rs
[INS]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/inspect.rs
[IS]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/inspect.rs
[COMP]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/compare.rs
[CS]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/compare.rs
[FILE]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/file.rs
[FILES]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/file.rs
[SAVE]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/save.rs
[STEP]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/kernel/src/step_out.rs
[MCPDOC]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/docs/mcp.md
[MCP]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/mcp/src/tools.rs
[PARAM]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/params.rs
[TYPES]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/types.rs
[LEGACY]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/unconverted.txt
[SCH]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/mod.rs
[IO]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/io/src/design.rs
[PLUGIN]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/plugin_data.rs
[CONFIG]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/config.rs
[RULE]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/config_rules.rs
[CONFIGDOC]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/docs/configurations.md
[META]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/metadata.rs
[CERTSTYLE]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/config_styles.rs
[CERT]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/config_certificates.rs
[DATUM]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/construct.rs
[SD]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/sketch_dimension.rs
[STEPDOC]: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/docs/book/src/guide/files.md
[OFFICIAL]: https://docs.solidcraft.io/reference/commands.html

The public **[SolidCraft command reference][OFFICIAL]** is useful for navigation, but the tagged Rust command/specification files above are the version-specific basis for this report.
