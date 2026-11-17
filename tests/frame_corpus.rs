//! Offline coverage for the frame-adaptation corpus items
//! (`corpus/frame-*`): the sign-normalizing equation chain, per-sketch map
//! state with restore-on-reference, and the non-permutation omission path
//! (named reasons plus dead equation slots). Everything here is offline;
//! the seat proves replay later. The omission path itself is unreachable on
//! the owner's seat (every origin plane there publishes a permutation map),
//! which is why its assertions live here and the bounded-support caveat
//! lives in the item's expect.txt.

use std::fs;
use std::path::{Path, PathBuf};

use craft_bridge::build::{self, BuildOptions};
use craft_bridge::limits::Limits;
use craft_bridge::pack::{self, PackOptions};
use craft_bridge::profile;
use craft_bridge::replay::{self, PlanStatus};
use craft_bridge::zipio;

const SIGN_CHAIN: &str = include_str!("fixtures/frame-sign-chain.json");
const TWO_MAPS: &str = include_str!("fixtures/frame-two-maps.json");
const OMISSION: &str = include_str!("fixtures/frame-omission.json");
const STEP_FIXTURE: &str = include_str!("fixtures/part.step");

const ITEMS: [(&str, &str); 3] = [
    ("frame-sign-chain", SIGN_CHAIN),
    ("frame-two-maps", TWO_MAPS),
    ("frame-omission", OMISSION),
];

struct Case {
    dir: tempfile::TempDir,
    artifact: PathBuf,
}

impl Case {
    fn new(recipe: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let recipe_path = dir.path().join("recipe.json");
        let step_path = dir.path().join("part.step");
        // The artifact basename enters the generated macro text, so test
        // artifacts keep the corpus name: byte equality needs `item.craftx`.
        let artifact = dir.path().join("item.craftx");
        fs::write(&recipe_path, recipe).unwrap();
        fs::write(&step_path, STEP_FIXTURE).unwrap();
        pack::pack(
            &PackOptions {
                recipe: &recipe_path,
                step: &step_path,
                out: &artifact,
                warnings: None,
                name: None,
                overwrite: false,
                snapshots: Vec::new(),
                final_snapshot: None,
            },
            &Limits::default(),
        )
        .unwrap();
        Case { dir, artifact }
    }

    fn build_to(&self, out_name: &str) -> String {
        let out = self.dir.path().join(out_name);
        build::build(
            &BuildOptions {
                artifact: &self.artifact,
                out: &out,
                plan: None,
                plan_text: None,
                overwrite: false,
            },
            &Limits::default(),
        )
        .unwrap();
        fs::read_to_string(&out).unwrap()
    }
}

/// The region between a `Function <name>`/`Sub <name>` header and its end.
fn proc_region<'a>(text: &'a str, name: &str) -> &'a str {
    let start = text
        .find(&format!("Function {name}("))
        .or_else(|| text.find(&format!("Sub {name}(")))
        .unwrap_or_else(|| panic!("no procedure {name}"));
    let end = text[start..]
        .find("End Function")
        .or_else(|| text[start..].find("End Sub"))
        .expect("the procedure ends")
        + start;
    &text[start..end]
}

// ---- the sign chain (corpus/frame-sign-chain) ----

#[test]
fn sign_chain_equations_flow_through_slotrefs() {
    let text = Case::new(SIGN_CHAIN).build_to("sign.swb");
    // The upstream dimension registers slot 0 WITH its runtime sign: the
    // sign is resolved from what the created dimension measures against the
    // source value, and it lands in the equation RHS, the expected value
    // and the registry alike. Whether the seat reports signed or unsigned
    // dimension values is a seat question; the generated coverage pins both
    // branches and the seat check (expect.txt) reads the stored equation.
    assert!(
        text.contains(
            "If Not AddDimEquation(dm, \"40mm\", signVal * 0.04, 0, signVal, skName"
        ),
        "the upstream dimension does not register its slot with the runtime sign"
    );
    for needle in ["signVal = 1#", "signVal = -1#", "but the source value is"] {
        assert!(text.contains(needle), "sign resolution lost: {needle}");
    }
    let add_eq = proc_region(&text, "AddDimEquation");
    assert!(
        add_eq.contains("regSign(slot) = signVal"),
        "AddDimEquation does not record the sign in the registry"
    );
    assert!(
        add_eq.contains("If signVal < 0 Then") && add_eq.contains("-1 * ("),
        "the sign is not folded into the stored equation text"
    );
    // Downstream equations reference the upstream slots through SlotRef -
    // two hops: slot 0 -> slot 1 -> slot 2.
    assert!(
        text.contains("AddDimEquation(dm, SlotRef(0) & \" * 2\", signVal * 0.08, 1, signVal, skName"),
        "the first downstream equation does not reference the upstream slot"
    );
    assert!(
        text.contains("AddDimEquation(dm, SlotRef(1) & \" / 8\", signVal * 0.01, 2, signVal, skName"),
        "the second downstream equation does not reference the mid-chain slot"
    );
    // SlotRef normalizes the upstream sign back to source semantics, so a
    // parameter edit keeps meaning what the source meant through the whole
    // chain (regSign/SlotRef end to end).
    let slot_ref = proc_region(&text, "SlotRef");
    assert!(
        slot_ref.contains("If regSign(slot) < 0 Then"),
        "SlotRef does not consult the registered sign"
    );
    assert!(
        slot_ref.contains("SlotRef = \"(-1*\" & \"\"\"\" & regName(slot) & \"\"\"\" & \")\""),
        "SlotRef does not normalize a negative upstream sign"
    );
    // Fail closed on dead slots before every downstream equation.
    for slot in [0, 1] {
        assert!(
            text.contains(&format!(
                "If regName({slot}) = \"\" Then gMsg = \"the parameter slot {slot} is not bound (its dimension was omitted or failed)\": GoTo DimFail"
            )),
            "no dead-slot guard on slot {slot}"
        );
    }
}

// ---- per-slot map state and restore-on-reference ----

#[test]
fn two_maps_keep_per_slot_state_and_restore_on_reference() {
    let text = Case::new(TWO_MAPS).build_to("maps.swb");
    // Each sketch publishes its own map slot from its own source frame.
    assert!(
        text.contains("AcceptAndPublish(1, \"YZ\", gProbeReason)"),
        "sketch 1 does not publish the YZ map into its own slot"
    );
    assert!(
        text.contains("AcceptAndPublish(2, \"XZ\", gProbeReason)"),
        "sketch 2 does not publish the XZ map into its own slot"
    );
    // Geometry is drawn through the sketch's own map, never the latest one.
    assert!(
        proc_region(&text, "DrawGeometry1").contains("MapU(1,"),
        "sketch 1 geometry does not use slot 1's map"
    );
    assert!(
        proc_region(&text, "DrawGeometry2").contains("MapU(2,"),
        "sketch 2 geometry does not use slot 2's map"
    );
    // Restore-on-reference: the extrude (step 3) references the FIRST
    // sketch and runs against that sketch's state while slot 2 holds a
    // different map - never its own step index.
    let extrude = proc_region(&text, "BuildExtrude3");
    assert!(
        extrude.contains("gMapValid(1)") && extrude.contains("gMapSide(1)"),
        "the extrude does not consult the referenced sketch's map slot"
    );
    assert!(
        extrude.contains("SelectByID2(skName1,"),
        "the extrude does not select the referenced sketch's identity"
    );
    assert!(
        !extrude.contains("skName3")
            && !extrude.contains("gMapSide(3)")
            && !extrude.contains("gMapValid(3)"),
        "the extrude consults its own step index instead of the reference"
    );
    // Its depth equation reads the first sketch's registered dimension -
    // registry state is per slot across features, guarded dead-closed.
    assert!(
        extrude.contains("AddDimEquation(dm, SlotRef(0), 0.03, 4, 1#)"),
        "the extrude depth equation does not reference the earlier sketch's slot"
    );
    assert!(
        extrude.contains("If regName(0) = \"\" Then"),
        "the cross-feature equation has no dead-slot guard"
    );
    // Sketch deletion invalidates, re-probes and rebinds its OWN slot only.
    let clean1 = proc_region(&text, "CleanSketch1");
    let clean2 = proc_region(&text, "CleanSketch2");
    assert!(
        clean1.contains("ClearMap 1") && clean1.contains("AcceptAndPublish(1, \"YZ\""),
        "CleanSketch1 does not own slot 1 end to end"
    );
    assert!(
        !clean1.contains("ClearMap 2") && !clean1.contains("AcceptAndPublish(2"),
        "CleanSketch1 touches slot 2"
    );
    assert!(
        clean2.contains("ClearMap 2") && clean2.contains("AcceptAndPublish(2, \"XZ\""),
        "CleanSketch2 does not own slot 2 end to end"
    );
    assert!(
        !clean2.contains("ClearMap 1") && !clean2.contains("AcceptAndPublish(1"),
        "CleanSketch2 touches slot 1"
    );
}

// ---- the non-permutation omission path ----

#[test]
fn omission_blocks_are_named_and_fail_closed_on_dead_slots() {
    let text = Case::new(OMISSION).build_to("omit.swb");
    // Every axis-dependent relation and dimension omits with a named reason
    // when the map is not a permutation - a successful skip (the branch
    // carries no failure goto), terminating by construction: the class
    // check is deterministic and the item is never re-attempted as a
    // failure.
    for reason in [
        "relation line-horizontal 0: the SC u axis is not axis-aligned in this sketch frame (non-permutation map); ",
        "relation line-vertical 1: the SC v axis is not axis-aligned in this sketch frame (non-permutation map); ",
        "relation points-horizontal 8-9: the SC u axis is not axis-aligned in this sketch frame (non-permutation map); ",
        "relation points-vertical 0-5: the SC v axis is not axis-aligned in this sketch frame (non-permutation map); ",
        "dimension DistanceX 0-1: the SC u axis is not axis-aligned in this sketch frame (non-permutation map); ",
        "dimension DistanceY 0-5: the SC v axis is not axis-aligned in this sketch frame (non-permutation map); ",
    ] {
        assert!(text.contains(reason), "omission reason lost: {reason}");
    }
    assert_eq!(
        text.matches("gOmit = gOmit &").count(),
        6,
        "omission reasons are not exactly the six axis-dependent items"
    );
    // A downstream equation over an omitted dimension's slot reaches a
    // dead-slot guard and fails closed (GoTo DimFail): slot 0 is referenced
    // by two downstream equations, slot 1 by one.
    assert_eq!(
        text.matches("If regName(0) = \"\" Then").count(),
        2,
        "slot 0 must carry a dead-slot guard for each downstream equation"
    );
    assert_eq!(
        text.matches("If regName(1) = \"\" Then").count(),
        1,
        "slot 1 must carry a dead-slot guard"
    );
    assert!(
        text.contains("the parameter slot 0 is not bound (its dimension was omitted or failed)"),
        "the dead-slot refusal lost its named reason"
    );
    // Failure routing: the first dimension pass fails into the restore
    // path; the restore rebuilds and RE-RUNS the dimensions, and that retry
    // routes to failure. On a non-permutation frame this fixture therefore
    // reports FAILED (the same deterministic omission leaves the same dead
    // slots) - never a silent success and never a successful degrade.
    let sketch = proc_region(&text, "BuildSketch1");
    assert!(
        sketch.contains("If Not Dims1() Then GoTo Degrade"),
        "the first dimension pass does not route to the restore path"
    );
    assert!(
        sketch.contains("If Not CleanSketch1() Then GoTo SketchFailed"),
        "the restore path does not fail closed on a refused restore"
    );
    assert!(
        sketch.contains("If Not Dims1() Then GoTo SketchFailed"),
        "the post-restore retry does not route a repeated failure to failed"
    );
    // The degraded-on-omission tail covers the omission-only outcome (a
    // sketch whose omitted slots nothing downstream references). This
    // fixture's outcome is the failure path above, not this tail.
    assert!(
        sketch.contains("If gOmit <> \"\" Then") && sketch.contains("BuildSketch1 = \"degraded\""),
        "the omission-only degraded tail is missing"
    );
}

// ---- the emitted-VBA gates ----

#[test]
fn frame_items_pass_the_emitted_vba_scans() {
    for (name, recipe) in ITEMS {
        let text = Case::new(recipe).build_to("scan.swb");
        assert_vba_literals_balance(&text, name);
        assert_vba_structure(&text, name);
        assert_no_intrinsic_shadowing(&text, name);
        assert_no_executable_residue(&text, name);
        assert_state_declared(&text, name);
    }
}

#[test]
fn every_local_is_declared_before_use() {
    // The offline compile contract (`Option Explicit`): every identifier a
    // procedure uses must be declared there, at module level, or be the
    // procedure itself. Regression coverage for the generator defect the
    // 2026-11-16 byte reviews found: the Dims template declared `v2` but
    // the length behaviour assigns `v` (src/swb.rs:1484/1877) - a compile
    // error the seat reports only as a bare invocation failure with no
    // report (docs/seat-acceptance.md "Runner quirks"). Scoped to these
    // items; the tracer family's macros carry that defect until the
    // emitter-owning stream fixes the template and regenerates them.
    for (name, recipe) in ITEMS {
        let text = Case::new(recipe).build_to("locals.swb");
        assert_locals_declared(&text, name);
    }
    // Negative control: the scan must actually catch an undeclared local
    // (the exact defect class it exists for).
    let broken = "Sub Build1()\n    Dim ok As Boolean\n    v = 1#\nEnd Sub\n";
    assert!(
        std::panic::catch_unwind(|| assert_locals_declared(broken, "negative-control")).is_err(),
        "the locals scan misses undeclared locals"
    );
}

// ---- the committed corpus artifacts and their pins ----

#[test]
fn corpus_pins_match_the_committed_artifacts() {
    for (name, recipe) in ITEMS {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("corpus")
            .join(name);
        let expect = fs::read_to_string(dir.join("expect.txt")).unwrap();
        let committed_swb = fs::read(dir.join("item.swb")).unwrap();
        let committed_craftx = dir.join("item.craftx");

        // A fresh pack+build of the fixture reproduces the committed macro
        // byte for byte (the artifact basename enters the macro text, so
        // the Case artifact is `item.craftx` like the corpus one).
        assert_eq!(
            Case::new(recipe).build_to("check.swb").as_bytes(),
            committed_swb.as_slice(),
            "{name}: the committed macro is stale against its fixture"
        );
        // The committed archive carries exactly the fixture recipe.
        let mut archive = zipio::open_archive(&committed_craftx, &Limits::default()).unwrap();
        let mut budget = Limits::default().max_total_bytes;
        let packed_recipe = zipio::read_member(
            &mut archive,
            profile::RECIPE,
            &Limits::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            packed_recipe,
            recipe.as_bytes(),
            "{name}: the committed archive's recipe differs from the fixture"
        );
        // Building the committed archive reproduces the committed macro.
        let out = dir.join("rebuilt.swb");
        build::build(
            &BuildOptions {
                artifact: &committed_craftx,
                out: &out,
                plan: None,
                plan_text: None,
                overwrite: false,
            },
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(
            fs::read(&out).unwrap(),
            committed_swb,
            "{name}: building the committed archive does not reproduce the committed macro"
        );
        fs::remove_file(&out).unwrap();
        // The expect.txt pin is the sha256 of exactly these macro bytes.
        let pin = expect
            .lines()
            .find_map(|l| l.strip_prefix("sha256="))
            .unwrap_or_else(|| panic!("{name}: expect.txt has no sha256 pin"));
        assert_eq!(
            sha256_hex(&committed_swb),
            pin,
            "{name}: the sha256 pin does not match the committed macro"
        );
        // reference.step mirrors the placeholder convention.
        assert_eq!(
            fs::read(dir.join("reference.step")).unwrap(),
            STEP_FIXTURE.as_bytes(),
            "{name}: reference.step is not the placeholder body"
        );
    }
}

#[test]
fn frame_plans_are_mapped_with_only_units_recorded() {
    for (name, recipe) in ITEMS {
        let value: serde_json::Value = serde_json::from_str(recipe).unwrap();
        craft_bridge::recipe::validate(
            serde_json::to_string(&value).unwrap().as_bytes(),
            &Limits::default(),
        )
        .unwrap();
        let steps = replay::plan(
            &value,
            &Limits::default(),
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        for step in &steps {
            match step.command.as_str() {
                "document.units" => assert_eq!(
                    step.status,
                    PlanStatus::NotExecuted,
                    "{name}: units must only be recorded"
                ),
                _ => assert_eq!(
                    step.status,
                    PlanStatus::Mapped,
                    "{name}: step {} is not mapped: {}",
                    step.index,
                    step.detail
                ),
            }
        }
    }
}

// ---- the scans (kept in step with tests/build_tests.rs) ----

fn code_only(line: &str) -> String {
    let mut code = String::new();
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                } else {
                    in_string = false;
                }
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '\'' {
            break;
        } else {
            code.push(c);
        }
    }
    code
}

fn assert_vba_literals_balance(text: &str, what: &str) {
    for (i, line) in text.lines().enumerate() {
        let mut in_string = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_string {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next(); // "" = one literal quote
                    } else {
                        in_string = false;
                    }
                }
            } else if c == '"' {
                in_string = true;
            } else if c == '\'' {
                break; // comment: rest of line is free text
            }
        }
        assert!(
            !in_string,
            "{what}: unterminated VBA string literal at line {}: {}",
            i + 1,
            line
        );
    }
}

fn assert_no_executable_residue(text: &str, what: &str) {
    for (i, line) in text.lines().enumerate() {
        let code = code_only(line);
        assert!(
            !code.contains('\\'),
            "{what}: executable residue at line {}: {}",
            i + 1,
            line
        );
    }
}

fn assert_no_intrinsic_shadowing(text: &str, what: &str) {
    const INTRINSICS: &[&str] = &[
        "sgn", "abs", "val", "mid", "left", "right", "len", "asc", "chr", "instr", "date", "time",
        "now", "timer", "format", "string", "array", "filter", "split", "join", "lcase", "ucase",
        "trim", "str", "fix", "int", "round", "sin", "cos", "tan", "log", "exp", "hex", "oct",
        "rgb", "environ", "shell", "name", "kill", "width", "spc", "tab", "stop", "line", "open",
        "close", "input", "print",
    ];
    let check = |name: &str, at: usize| {
        assert!(
            !INTRINSICS.contains(&name.to_lowercase().as_str()),
            "{what}: identifier {name} shadows a VBA intrinsic (line {at})"
        );
    };
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        let up = t.to_uppercase();
        if up.starts_with("DIM ") || up.starts_with("CONST ") {
            for part in t[4..].split(',') {
                if let Some(m) = part.split_whitespace().next() {
                    let name = m.split('(').next().unwrap_or("");
                    if !name.is_empty() {
                        check(name, i + 1);
                    }
                }
            }
        }
        for kw in ["FUNCTION ", "SUB "] {
            if up.starts_with(kw) {
                if let Some(m) = t[kw.len()..].split('(').next() {
                    let name = m.trim();
                    if !name.is_empty() {
                        check(name, i + 1);
                    }
                }
                if let Some(plist) = t.split('(').nth(1) {
                    for p in plist.split(',') {
                        // The parameter NAME is the first non-keyword token;
                        // the type follows "As".
                        for tok in p.split_whitespace() {
                            let low = tok.to_lowercase();
                            if low == "byval"
                                || low == "byref"
                                || low == "optional"
                                || low == "paramarray"
                            {
                                continue;
                            }
                            let name = tok.trim_end_matches(')');
                            if !name.is_empty() {
                                check(name, i + 1);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn assert_state_declared(text: &str, what: &str) {
    let mut declared: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in text.lines() {
        let t = line.trim();
        let up = t.to_uppercase();
        if up.starts_with("DIM ") || up.starts_with("CONST ") {
            for part in t[4..].split(',') {
                let name = part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('(')
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    declared.insert(name.to_lowercase());
                }
            }
        }
    }
    for (i, line) in text.lines().enumerate() {
        let code = code_only(line);
        let mut ident = String::new();
        for c in code.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_alphanumeric() || c == '_' {
                ident.push(c);
            } else {
                if ident.starts_with('g')
                    && ident.len() > 1
                    && ident.as_bytes()[1].is_ascii_uppercase()
                {
                    assert!(
                        declared.contains(&ident.to_lowercase()),
                        "{what}: {ident} is used at line {} but never declared",
                        i + 1
                    );
                }
                ident.clear();
            }
        }
    }
}

/// The `Option Explicit` contract: every identifier used inside a procedure
/// must be that procedure's local/parameter, a module-level declaration, a
/// label, or a procedure name. Tokens around `.` are qualifiers or enum
/// prefixes and are skipped; VBA's own vocabulary is the allowlist.
fn assert_locals_declared(text: &str, what: &str) {
    // VBA keywords, intrinsic functions and intrinsic constants: never a
    // user identifier in these macros.
    const VBA_WORDS: &[&str] = &[
        "dim",
        "const",
        "as",
        "set",
        "let",
        "get",
        "call",
        "new",
        "nothing",
        "true",
        "false",
        "empty",
        "null",
        "if",
        "then",
        "else",
        "elseif",
        "end",
        "select",
        "case",
        "for",
        "next",
        "to",
        "step",
        "each",
        "in",
        "do",
        "while",
        "loop",
        "until",
        "wend",
        "with",
        "exit",
        "goto",
        "on",
        "error",
        "resume",
        "function",
        "sub",
        "property",
        "private",
        "public",
        "static",
        "optional",
        "byref",
        "byval",
        "paramarray",
        "not",
        "and",
        "or",
        "is",
        "mod",
        "xor",
        "eqv",
        "imp",
        "boolean",
        "byte",
        "integer",
        "long",
        "single",
        "double",
        "string",
        "variant",
        "object",
        "date",
        "any",
        "sgn",
        "abs",
        "val",
        "mid",
        "left",
        "right",
        "len",
        "asc",
        "chr",
        "instr",
        "time",
        "now",
        "timer",
        "format",
        "string",
        "array",
        "filter",
        "split",
        "join",
        "lcase",
        "ucase",
        "trim",
        "str",
        "fix",
        "int",
        "round",
        "sin",
        "cos",
        "tan",
        "log",
        "exp",
        "hex",
        "oct",
        "rgb",
        "environ",
        "shell",
        "name",
        "kill",
        "width",
        "spc",
        "tab",
        "stop",
        "line",
        "open",
        "close",
        "input",
        "print",
        "err",
        "ubound",
        "lbound",
        "isarray",
        "isnumeric",
        "isempty",
        "isnull",
        "isobject",
        "cint",
        "clng",
        "cdbl",
        "csng",
        "cstr",
        "cbool",
        "cvar",
        "cdate",
        "vbcrlf",
        "vbcr",
        "vblf",
        "vbtab",
        "vbobjecterror",
        "vbtextcompare",
        "msgbox",
        "inputbox",
        "typename",
        "createobject",
        "getobject",
        "chrw",
        "ascw",
        "sqr",
        "replace",
        "freefile",
        "output",
        "append",
    ];
    let is_word = |t: &str| VBA_WORDS.contains(&t.to_lowercase().as_str());
    let decl_names = |line: &str| -> Vec<String> {
        let t = line.trim();
        // Strip the Dim/Const keyword; a fixed offset is wrong for Const.
        let rest = match t.find(' ') {
            Some(sp) => &t[sp..],
            None => return Vec::new(),
        };
        rest.split(',')
            .filter_map(|part| {
                let name = part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('(')
                    .next()
                    .unwrap_or("");
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_lowercase())
                }
            })
            .collect()
    };

    // Pass 1: procedure spans, names, parameters, labels and declarations.
    let mut proc_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut module_decls: std::collections::HashSet<String> = std::collections::HashSet::new();
    // (proc name, decls, labels) per procedure; None = module level.
    let mut owners: Vec<Option<usize>> = Vec::new();
    let mut procs: Vec<(String, std::collections::HashSet<String>)> = Vec::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        let t = line.trim();
        let up = t.to_uppercase();
        let code = code_only(line);
        let cu = code.trim().to_uppercase();
        if cu.starts_with("FUNCTION ") || cu.starts_with("SUB ") {
            let name = t
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .to_lowercase();
            proc_names.insert(name.clone());
            let mut decls: std::collections::HashSet<String> = std::collections::HashSet::new();
            if let Some(plist) = t.split('(').nth(1) {
                for p in plist.split(',') {
                    for tok in p.split_whitespace() {
                        let low = tok.to_lowercase();
                        if low == "byval"
                            || low == "byref"
                            || low == "optional"
                            || low == "paramarray"
                        {
                            continue;
                        }
                        decls.insert(tok.trim_end_matches(')').to_lowercase());
                        break;
                    }
                }
            }
            procs.push((name, decls));
            current = Some(procs.len() - 1);
            owners.push(current);
            continue;
        }
        if cu.starts_with("END FUNCTION") || cu.starts_with("END SUB") {
            current = None;
            owners.push(None);
            continue;
        }
        owners.push(current);
        if up.starts_with("DIM ") || up.starts_with("CONST ") {
            let names = decl_names(t);
            match current {
                Some(pi) => procs[pi].1.extend(names),
                None => module_decls.extend(names),
            }
            continue;
        }
        if let Some(pi) = current {
            let c = code.trim();
            if let Some(label) = c.split(':').next() {
                if c.contains(':')
                    && label
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    && !label.is_empty()
                {
                    procs[pi].1.insert(label.to_lowercase());
                }
            }
        }
    }

    // Pass 2: every used identifier is declared somewhere legitimate.
    for (i, line) in text.lines().enumerate() {
        let Some(pi) = owners.get(i).copied().flatten() else {
            continue;
        };
        let code = code_only(line);
        let cu = code.trim().to_uppercase();
        if cu.starts_with("FUNCTION ")
            || cu.starts_with("SUB ")
            || cu.starts_with("DIM ")
            || cu.starts_with("CONST ")
            || cu.starts_with("END FUNCTION")
            || cu.starts_with("END SUB")
        {
            continue;
        }
        let chars: Vec<char> = code.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j].is_ascii_alphabetic() || chars[j] == '_' {
                let start = j;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                // Word boundaries: "1E+100" must not yield "e".
                if start > 0
                    && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_')
                {
                    continue;
                }
                let after_dot = start > 0 && chars[start - 1] == '.';
                let before_dot = j < chars.len() && chars[j] == '.';
                if after_dot || before_dot {
                    continue;
                }
                let token: String = chars[start..j].iter().collect();
                let low = token.to_lowercase();
                if is_word(&low) {
                    continue;
                }
                if procs[pi].1.contains(&low) || module_decls.contains(&low) {
                    continue;
                }
                if proc_names.contains(&low) {
                    continue;
                }
                panic!(
                    "{what}: {token} is used in {} at line {} but declared nowhere",
                    procs[pi].0,
                    i + 1
                );
            } else {
                j += 1;
            }
        }
    }
}

fn assert_vba_structure(text: &str, what: &str) {
    let mut block_ifs = 0i32;
    let mut end_ifs = 0i32;
    let mut procs = 0i32;
    let mut end_procs = 0i32;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let proc_names: Vec<String> = text
        .lines()
        .filter_map(|l| {
            let u = l.trim().to_uppercase();
            if u.starts_with("FUNCTION ") || u.starts_with("SUB ") {
                Some(
                    l.split_whitespace()
                        .nth(1)
                        .unwrap_or("")
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect();
    for line in text.lines() {
        let mut code = String::new();
        let mut in_string = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_string {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                    } else {
                        in_string = false;
                    }
                }
            } else if c == '"' {
                in_string = true;
            } else if c == '\'' {
                break;
            } else {
                code.push(c);
            }
        }
        let t = code.trim();
        let up = t.to_uppercase();
        if up.starts_with("FUNCTION ") || up.starts_with("SUB ") {
            procs += 1;
            seen.clear();
        }
        if up.starts_with("END FUNCTION") || up.starts_with("END SUB") {
            end_procs += 1;
            seen.clear();
        }
        for decl in ["DIM ", "CONST "] {
            if up.starts_with(decl) {
                for part in t[decl.len()..].split(',') {
                    let name = part.split_whitespace().next().unwrap_or("").to_lowercase();
                    if !name.is_empty() && !seen.insert(name.clone()) {
                        panic!("{what}: duplicate declaration of {name}");
                    }
                }
            }
        }
        // Case-insensitive shadowing: VBA identifiers fold case, so a local
        // named like a module procedure makes calls to it unresolvable.
        for proc in &proc_names {
            if seen.contains(&proc.to_lowercase()) {
                panic!("{what}: a local name shadows the procedure {proc}");
            }
        }
        // A bare "Name()" statement is not a valid VBA procedure call:
        // parens require Call or an assignment context.
        if t.ends_with("()") && !t.contains(' ') && !t.contains('=') {
            panic!("{what}: bare parenthesized Sub call is invalid VBA: {t}");
        }
        if up.starts_with("IF ") && up.ends_with("THEN") {
            block_ifs += 1;
        }
        if up.starts_with("END IF") {
            end_ifs += 1;
        }
    }
    assert_eq!(procs, end_procs, "{what}: unbalanced Function/Sub blocks");
    assert_eq!(
        block_ifs, end_ifs,
        "{what}: block-If/End-If imbalance - fused or orphaned statements"
    );
}

// ---- a minimal SHA-256 (FIPS 180-4) to verify the expect.txt pins without
// ---- adding a dependency to the shared manifest ----

fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().enumerate().take(16) {
            *word = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[test]
fn sha256_matches_the_known_answer() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// ---- every committed macro, generated or hand-written ----

#[test]
fn every_committed_macro_passes_the_scans() {
    // The full scan set over EVERY committed macro byte stream, not only
    // the recipes the other tests rebuild (plan review): the regenerated
    // corpus bytes and the new orientation probe (hand-written, same
    // Option Explicit compile contract) must pass the literal-balance,
    // structure, intrinsic-shadowing, residue and declaration scans like
    // the generated macros. A compile failure presents on the seat only as
    // a bare invocation failure with no report - these scans are the only
    // early warning.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let corpus = root.join("corpus");
    let mut targets: Vec<(String, String)> = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap()).collect();
    dirs.sort_by_key(|e| e.file_name());
    for entry in dirs {
        let item = entry.path().join("item.swb");
        if item.is_file() {
            let name = format!("corpus/{}", entry.file_name().to_string_lossy());
            targets.push((name, fs::read_to_string(&item).unwrap()));
        }
    }
    targets.push((
        "corpus/compat-probe/probe-primorient".to_string(),
        fs::read_to_string(corpus.join("compat-probe").join("probe-primorient.swb")).unwrap(),
    ));
    assert!(
        targets.len() > 3,
        "the corpus scan found no committed macros"
    );
    for (name, text) in &targets {
        assert_vba_literals_balance(text, name);
        assert_vba_structure(text, name);
        assert_no_intrinsic_shadowing(text, name);
        assert_no_executable_residue(text, name);
        assert_state_declared(text, name);
        assert_locals_declared(text, name);
    }
}
