//! Offline tests for the seat tooling (tools/): the check-part decision
//! logic (mirrored from tools/check-part-logic.vbs and pinned to its text),
//! the checker's fail-closed contract, the harness's mass gate, and the
//! real-part runner's provenance gates (the AGENTS.md real-part
//! private-run exception). Everything here is synthetic and authored for
//! this repository - never anything from uploads/ or runs/.

use std::process::Command;

const LOGIC: &str = include_str!("../tools/check-part-logic.vbs");
const CHECKER: &str = include_str!("../tools/check-part.vbs");
const HARNESS: &str = include_str!("../tools/seat-harness.ps1");
const RUNNER: &str = include_str!("../tools/seat-run-real.ps1");
const RUNNER_TESTS: &str = include_str!("../tools/seat-run-real.tests.ps1");
const LOGIC_TESTS: &str = include_str!("../tools/check-part-logic.tests.vbs");

// ---- the decision logic, mirrored ----
// Mirrors tools/check-part-logic.vbs exactly; the pins below tie the two
// together. None = the value is missing/unreadable text.

fn valid_positive(v: Option<f64>) -> bool {
    matches!(v, Some(x) if x.is_finite() && x > 0.0)
}

fn valid_nonneg(v: Option<f64>) -> bool {
    matches!(v, Some(x) if x.is_finite() && x >= 0.0)
}

/// `MassLines`: pass iff every value validates and
/// |rebuilt - reference| <= max(rel_tol * reference, abs_tol).
fn mass_verdict(
    rebuilt: Option<f64>,
    reference: Option<f64>,
    rel: Option<f64>,
    abs: Option<f64>,
) -> &'static str {
    // volume_tol_abs is optional; its documented default is 0.
    let abs = abs.or(Some(0.0));
    if !(valid_positive(rebuilt)
        && valid_positive(reference)
        && valid_nonneg(rel)
        && valid_nonneg(abs))
    {
        return "fail";
    }
    let (r, f) = (rebuilt.unwrap(), reference.unwrap());
    let limit = (rel.unwrap() * f).max(abs.unwrap());
    if (r - f).abs() <= limit {
        "pass"
    } else {
        "fail"
    }
}

/// `RefImportLines`: the expected count validates, the document is a
/// part, and the found solid body count matches exactly. `doc_type` is
/// "part" / "assembly" / "drawing" / "other" / "unreadable".
fn ref_import_verdict(expected: Option<f64>, doc_type: &str, solids: Option<f64>) -> &'static str {
    let Some(expected) = expected.filter(|e| e.is_finite() && *e >= 1.0 && e.fract() == 0.0) else {
        return "fail";
    };
    if doc_type != "part" {
        return "fail";
    }
    let Some(solids) = solids.filter(|s| s.is_finite() && *s >= 1.0 && s.fract() == 0.0) else {
        return "fail";
    };
    if solids == expected {
        "pass"
    } else {
        "fail"
    }
}

/// Mirrors `ParsePeriod` exactly: a strict period-decimal shape, then the
/// locale-independent Val conversion. Comma-decimal text (or any other
/// shape) is refused whatever the host locale says.
fn parse_period(text: &str) -> Option<f64> {
    let s = text.trim();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    if i < b.len() && (b[i] == '+' || b[i] == '-') {
        i += 1;
    }
    let mut digits = 0i32;
    let mut dots = 0i32;
    let mut exp_seen = false;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            digits += 1;
        } else if c == '.' && !exp_seen {
            dots += 1;
            if dots > 1 {
                return None;
            }
        } else if (c == 'e' || c == 'E') && !exp_seen && digits > 0 {
            exp_seen = true;
            digits = 0;
            if i + 1 < b.len() && (b[i + 1] == '+' || b[i + 1] == '-') {
                i += 1;
            }
        } else {
            return None;
        }
        i += 1;
    }
    if digits == 0 {
        return None;
    }
    // Val reads a leading sign, a leading period and exponents; normalize
    // the two forms Rust's parser lacks before converting.
    let mut t = s.trim_start_matches('+').to_string();
    if t.starts_with('.') {
        t.insert(0, '0');
    }
    if t.starts_with("-.") {
        t = t.replacen("-.", "-0.", 1);
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[test]
fn locale_parsing_is_period_decimal_only() {
    assert_eq!(parse_period("0.01"), Some(0.01));
    assert_eq!(parse_period("  0.01  "), Some(0.01));
    assert_eq!(parse_period("-1E-06"), Some(-1e-6));
    assert_eq!(parse_period(".5"), Some(0.5));
    assert_eq!(parse_period("+.5"), Some(0.5));
    // The comma-decimal negative case: valid-looking text on a
    // non-English Windows must NOT parse here.
    assert_eq!(parse_period("0,01"), None);
    for junk in ["1.2.3", "1e", "abc", "", "+", "-", "1e+400"] {
        assert_eq!(parse_period(junk), None, "{junk} must be refused");
    }
}

/// `BodiesLines`: the rebuilt count matches the expected count exactly.
fn bodies_verdict(expected: Option<f64>, rebuilt: Option<f64>) -> &'static str {
    match (expected, rebuilt) {
        (Some(e), Some(r))
            if e.is_finite()
                && r.is_finite()
                && e >= 1.0
                && r >= 1.0
                && e.fract() == 0.0
                && r.fract() == 0.0
                && e == r =>
        {
            "pass"
        }
        _ => "fail",
    }
}

/// `PlacementLines`: the centroid distance is within the tolerance (which
/// is REQUIRED - there is no default).
fn placement_verdict(dist: Option<f64>, tol: Option<f64>) -> &'static str {
    if !(valid_nonneg(dist) && valid_nonneg(tol)) {
        return "fail";
    }
    if dist.unwrap() <= tol.unwrap() {
        "pass"
    } else {
        "fail"
    }
}

// ---- checker negative tests (fixture cases over the mirror) ----

#[test]
fn mass_verdict_fails_closed_on_missing_or_invalid_volumes() {
    let rel = Some(0.001);
    let abs = Some(0.0);
    for (rebuilt, reference, why) in [
        (None, Some(0.01), "missing rebuilt"),
        (Some(0.01), None, "missing reference"),
        (Some(0.0), Some(0.01), "nonpositive rebuilt"),
        (Some(-0.01), Some(0.01), "negative rebuilt"),
        (Some(0.01), Some(0.0), "nonpositive reference"),
        (Some(f64::NAN), Some(0.01), "non-finite rebuilt"),
        (Some(f64::INFINITY), Some(0.01), "infinite rebuilt"),
    ] {
        assert_eq!(
            mass_verdict(rebuilt, reference, rel, abs),
            "fail",
            "{why} must fail closed"
        );
    }
    // The tolerance values are validated at the tooling boundary too.
    assert_eq!(mass_verdict(Some(0.01), Some(0.01), None, abs), "fail");
    assert_eq!(
        mass_verdict(Some(0.01), Some(0.01), Some(-0.001), abs),
        "fail"
    );
    assert_eq!(
        mass_verdict(Some(0.01), Some(0.01), rel, Some(-1.0)),
        "fail"
    );
}

#[test]
fn mass_verdict_tolerance_edges_relative_vs_absolute() {
    // Exactly at the relative limit passes; past it fails.
    assert_eq!(
        mass_verdict(Some(0.01001), Some(0.01), Some(0.001), Some(0.0)),
        "pass"
    );
    assert_eq!(
        mass_verdict(Some(0.010011), Some(0.01), Some(0.001), Some(0.0)),
        "fail"
    );
    // At a tiny reference the relative bound collapses to 1e-9: the
    // absolute floor (5e-8) governs and admits 2e-8 but not 8e-8.
    assert_eq!(
        mass_verdict(
            Some(0.00000102),
            Some(0.000001),
            Some(0.001),
            Some(0.00000005)
        ),
        "pass"
    );
    assert_eq!(
        mass_verdict(
            Some(0.00000108),
            Some(0.000001),
            Some(0.001),
            Some(0.00000005)
        ),
        "fail"
    );
    // At a large reference the relative bound governs even with a floor.
    assert_eq!(
        mass_verdict(Some(1.0005), Some(1.0), Some(0.001), Some(0.00000005)),
        "pass"
    );
    assert_eq!(
        mass_verdict(Some(1.002), Some(1.0), Some(0.001), Some(0.00000005)),
        "fail"
    );
    // The floor can widen the limit where the relative bound alone fails.
    assert_eq!(
        mass_verdict(Some(1.002), Some(1.0), Some(0.001), Some(0.01)),
        "pass"
    );
}

#[test]
fn import_verdicts_name_assembly_surface_and_incomplete() {
    assert_eq!(ref_import_verdict(Some(1.0), "part", Some(1.0)), "pass");
    assert_eq!(ref_import_verdict(Some(1.0), "assembly", Some(3.0)), "fail");
    assert_eq!(ref_import_verdict(Some(1.0), "other", Some(1.0)), "fail");
    assert_eq!(
        ref_import_verdict(Some(1.0), "unreadable", Some(1.0)),
        "fail"
    );
    assert_eq!(ref_import_verdict(Some(1.0), "part", Some(0.0)), "fail");
    assert_eq!(ref_import_verdict(Some(3.0), "part", Some(2.0)), "fail");
    assert_eq!(ref_import_verdict(Some(1.0), "part", Some(3.0)), "fail");
    assert_eq!(ref_import_verdict(Some(1.5), "part", Some(1.0)), "fail");
    assert_eq!(ref_import_verdict(None, "part", Some(1.0)), "fail");
    assert_eq!(ref_import_verdict(Some(1.0), "part", None), "fail");
}

#[test]
fn body_and_placement_verdicts_run_alongside_volume() {
    assert_eq!(bodies_verdict(Some(1.0), Some(1.0)), "pass");
    assert_eq!(bodies_verdict(Some(1.0), Some(2.0)), "fail");
    assert_eq!(bodies_verdict(Some(1.0), None), "fail");
    assert_eq!(placement_verdict(Some(0.00001), Some(0.0001)), "pass");
    assert_eq!(placement_verdict(Some(0.001), Some(0.0001)), "fail");
    // The placement tolerance is required - no default.
    assert_eq!(placement_verdict(Some(0.001), None), "fail");
    assert_eq!(placement_verdict(None, Some(0.0001)), "fail");
}

// ---- the mirror is pinned to the VBS decision text ----

#[test]
fn the_decision_mirror_is_pinned_to_the_vbs_logic() {
    // Every Check call takes (name, condition, detail): a missing comma
    // made the suite fail to compile once, hiding a broken parser.
    for line in LOGIC_TESTS.lines() {
        if line.trim_start().starts_with("Check \"") {
            assert!(
                line.contains("\", "),
                "a Check call lost its name/condition comma: {line}"
            );
        }
    }
    for pin in [
        "limit = rel * refVol",
        "If absTol > limit Then limit = absTol",
        "If diff <= limit Then verdict = \"pass\"",
        "If diff <= limit Then",
        "check:mass=\" & verdict",
        "check:mass_vs_reference=\" & verdict",
    ] {
        assert!(
            LOGIC.contains(pin),
            "the VBS logic lost its pinned form: {pin}"
        );
    }
    for pin in [
        "imported as an assembly",
        "a surface import",
        "incomplete import",
        "solid bodies where ",
        "carries ",
    ] {
        assert!(
            LOGIC.contains(pin),
            "the import verdicts lost their named reasons: {pin}"
        );
    }
    // Acquisition (CentroidText/ComponentNumber) is fail closed with
    // named reasons and period-only string parsing (IsNumeric/CDbl are
    // locale-poisoned; Val does not exist in VBScript).
    for pin in [
        "component is an object",
        "string component is not period-decimal",
        "unsupported component type",
        "component is not finite",
        "unexpected shape",
        "is not numeric (vartype ",
    ] {
        assert!(
            LOGIC.contains(pin),
            "the acquisition lost a named reason: {pin}"
        );
    }
    // The STEP import uses the documented LoadFile3 shape (FileName,
    // ArgString, ImportData) - Boolean return, only [in] parameters, the
    // form this seat's VBScript can marshal (the legacy LoadFile4 shape's
    // ByRef Errors output raises Type mismatch here regardless of the
    // placeholder - the RunMacro2 ByRef Long precedent). ONE attempt (the
    // single-attempt discipline; the speculative marshalling ladder is
    // gone), the loaded document taken from ActiveDoc and
    // ownership-validated against the pre-import snapshot.
    assert!(
        CHECKER.contains("GetImportFileData")
            && CHECKER.contains("LoadFile3(")
            && CHECKER.contains("ImportStepDoc"),
        "the reference import lost its documented, diagnosed form"
    );
    assert!(
        !CHECKER.contains("app.LoadFile4("),
        "the uncallable ByRef-Errors load form survived"
    );
    // the attempt is ownership-reconciled against the snapshot
    assert!(
        CHECKER.contains("OwnershipReconciled") && CHECKER.contains("ambiguous ownership"),
        "the import attempt lost its ownership reconciliation"
    );
    assert!(
        CHECKER.contains("refusing to attribute it"),
        "the ActiveDoc handoff is not ownership-validated"
    );
    assert!(
        LOGIC.contains("component is an object that does not coerce"),
        "the object-coercion refusal lost its named reason"
    );
    assert!(
        !CHECKER.contains("Function CentroidText("),
        "the acquisition must live in the shared logic module (testable via cscript)"
    );
    // The distance computation is checked end to end (Sqr(0) raised in
    // the SW VBA host; a failed read must never coerce to zero).
    assert!(
        CHECKER.contains("the squared distance") && CHECKER.contains("the square root raised err "),
        "the distance computation lost its checked failure path"
    );
    // The distance block runs on EVERY path of the comparison branch
    // (it used to sit inside the reference-loaded branch: a failed
    // import left distText empty and the placement detail rendered with
    // an empty suffix).
    let acquire = CHECKER
        .find("refAcquireErr = CentroidText(mp, refCx")
        .expect("no reference centroid acquisition");
    let dist = CHECKER
        .find("distText = \"missing or unreadable\"")
        .expect("no distance-unavailable naming");
    let close = CHECKER
        .find("refCloseLines = \"\"")
        .expect("no close block");
    assert!(
        acquire < dist && dist < close,
        "the distance block does not sit between the acquisitions and the close"
    );
    // The validators fail closed with named reasons (the negative tests
    // above pin their behavior through the mirror).
    for pin in [
        "missing or unreadable",
        "nonpositive",
        "not numeric",
        "not finite",
        "not an integer",
    ] {
        assert!(
            LOGIC.contains(pin),
            "the validators lost a named reason: {pin}"
        );
    }
    // Locale-independent parsing (finding 3): the strict period-decimal
    // parser converts by locale-free arithmetic. VBScript has no Val (the
    // first attempt called it and silently failed every parse), and the
    // locale-sensitive members never parse again.
    assert!(
        LOGIC.contains("Function ParsePeriod")
            && LOGIC.contains("value = PeriodValue(s)")
            && LOGIC.contains("Function PeriodValue"),
        "the decision logic lost its locale-independent parser"
    );
    assert!(
        !LOGIC.contains("IsNumeric(") && !LOGIC.contains("CDbl(") && !LOGIC.contains("Val(s)"),
        "locale-sensitive or nonexistent numeric conversion survived in the decision logic"
    );
    // The solid=none contract: a no-solid item verifies the absence
    // two-way and never hides a broken read.
    for pin in [
        "solid=none",
        "check:solid=pass",
        "check:solid=fail",
        "absence unverifiable",
        "unexpected solid volume ",
    ] {
        assert!(
            LOGIC.contains(pin),
            "the decision logic lost its solid=none contract: {pin}"
        );
    }
    // The zero-body/surface branches are REACHED (finding 2): the found
    // count allows zero and names surface/empty imports.
    assert!(
        LOGIC.contains("Function FoundCountError") && LOGIC.contains("a surface import"),
        "the zero-body naming is unreachable"
    );
    let rf = &LOGIC[LOGIC
        .find("Function RefImportLines")
        .expect("no RefImportLines")..];
    assert!(
        rf.contains("FoundCountError(solidsText)"),
        "the found count is refused as invalid before the zero-body naming"
    );
    // Reference ownership (finding 4): a pre-existing title is named and
    // never closed.
    assert!(
        LOGIC.contains("Function IsPreExistingTitle"),
        "the ownership helper is missing"
    );
}

// ---- the checker's fail-closed contract ----

#[test]
fn the_mass_entry_is_never_absent() {
    // The required comparison entry is always emitted: the builders always
    // end with both verdicts, and the missing-logic path writes them by
    // hand as failures before quitting.
    let missing_logic = CHECKER
        .find("check-part-logic.vbs is missing")
        .expect("no missing-logic path");
    let region = &CHECKER[missing_logic..missing_logic + 400];
    assert!(
        region.contains("check:mass=fail") && region.contains("check:mass_vs_reference=fail"),
        "the missing-logic path does not fail closed with the required entries"
    );
    assert!(
        CHECKER.contains("ExecuteGlobal logicText"),
        "the checker does not load the shared decision logic"
    );
    assert!(
        LOGIC.contains("MassLines = out") && LOGIC.contains("check:mass_vs_reference="),
        "the comparison entry is conditional"
    );
    // The hardcoded relative tolerance is gone: volume_tol is honored.
    assert!(
        !CHECKER.contains("0.001") && !LOGIC.contains("0.001"),
        "a hardcoded 0.001 tolerance survived"
    );
    assert!(
        CHECKER.contains("volume_tol=") && LOGIC.contains("volume_tol"),
        "the documented volume_tol parameter is not read"
    );
    assert!(
        LOGIC.contains("volume_tol_abs") && LOGIC.contains("placement_tol"),
        "the documented expect parameters are missing from the decision logic"
    );
    assert!(
        CHECKER.contains("ref_bodies=") && CHECKER.contains("placement_tol="),
        "the documented expect parameters are not read by the checker"
    );
}

#[test]
fn checker_syntax_is_paired_and_acquisition_is_typed() {
    // The solid=none flag must be declared (Option Explicit turns an
    // undeclared variable into a swallowed runtime error that once
    // silently emptied the whole mass block), and both decision calls
    // must fail closed instead of leaving absent entries.
    assert!(
        CHECKER.contains("Dim skipVolume, noSolid, minFeatures"),
        "the checker lost its noSolid declaration"
    );
    for pin in ["the decision call failed: ", "check:mass=fail"] {
        assert!(
            CHECKER.contains(pin),
            "the checker lost its fail-closed decision-call guard: {pin}"
        );
    }
    // Finding 1: a single-line If...Then cannot pair with ElseIf - the
    // branches are block form now (the Windows syntax test parses the
    // whole file; this pins the offending shape out).
    assert!(
        !CHECKER.contains("Then docTypeText"),
        "a single-line If assignment is paired with ElseIf again"
    );
    assert!(
        CHECKER.contains("ElseIf dtRaw = 2 Then"),
        "the document-type block lost its paired branches"
    );
    // Finding 2: GetBodies2(BodyType, BVisibleOnly) with both required
    // arguments (public API docs: swSolidBody=0, swSheetBody=1) and typed
    // acquisition - an unexpected return shape is a named refusal, never
    // an invented count.
    assert!(
        CHECKER.contains("GetBodies2(0, False)") && CHECKER.contains("GetBodies2(1, False)"),
        "GetBodies2 is not called with its required arguments"
    );
    assert!(
        CHECKER.contains("Function CountText") && CHECKER.contains("unexpected return shape"),
        "the body counts are not proven typed"
    );
    assert!(
        LOGIC.contains("Function CentroidText") && LOGIC.contains("centerofmass"),
        "the centroid components are not proven typed"
    );
    assert!(
        LOGIC.contains("Function ComponentNumber") && LOGIC.contains("VarType(comp)"),
        "the component type gate is missing"
    );
    // capture-then-build: failure messages are built from plain
    // variables only - a raise inside message construction silently
    // skips the assignment and leaves the DEFAULT verdict text standing
    // (D01 run 15 evidence).
    assert!(
        LOGIC.contains("vtCap = VarType(") && LOGIC.contains("tnCap = TypeName("),
        "the acquisition messages are not capture-then-build"
    );
    assert!(
        LOGIC.contains("vartype ") && LOGIC.contains("IsArray"),
        "the acquisition lost its shape checks"
    );
    // Finding 4: the ownership snapshot precedes the import and a
    // pre-existing title refuses the close, named.
    let snap = CHECKER
        .find("preTitles = preTitles & t2")
        .expect("no pre-import snapshot");
    let load = CHECKER.find("LoadFile3(").expect("no import");
    assert!(
        snap < load,
        "the document snapshot is taken after the import"
    );
    assert!(
        CHECKER.contains("IsPreExistingTitle(preTitles, refTitle)")
            && CHECKER.contains("refusing to close a pre-existing document"),
        "a pre-existing document is not refused named"
    );
    assert!(
        CHECKER.contains("the pre-import document enumeration failed"),
        "an unestablishable ownership check does not fail closed"
    );
}

#[test]
fn the_reference_document_is_closed_with_the_verified_discipline() {
    // The checker opens the STEP reference and must close it the
    // CloseOwnedDoc way: a live enumeration identifying exactly the opened
    // object before the close, and proving its absence after. Every
    // failure path reports check:ref_close=fail and claims nothing.
    let start = CHECKER
        .find("Function VerifiedCloseLines")
        .expect("no verified reference close");
    let body = &CHECKER[start..];
    assert!(
        body.contains("check:ref_close=fail") && body.contains("check:ref_close=pass"),
        "the reference close carries no verdict"
    );
    assert!(
        body.contains("app.CloseDoc title"),
        "the reference document is never closed"
    );
    let before = body
        .find("cnt <> 1 Or Not isOurs")
        .expect("no pre-close ownership check");
    let close = body.find("app.CloseDoc title").expect("no close");
    let after = body.find("cnt <> 0").expect("no post-close verification");
    assert!(
        before < close && close < after,
        "the reference close is not verified before and after"
    );
    assert!(
        body.contains("On Error Resume Next"),
        "the close discipline does not trap its own calls"
    );
    // The skip path reports the reference close as skipped - never absent.
    assert!(
        CHECKER.contains("check:ref_close=skipped"),
        "the skip path leaves the reference close entry absent"
    );
}

#[test]
fn the_checker_seams_and_hands_off_exactly_the_owned_document() {
    // The pure/COM seam: the checker owns only COM acquisition.
    assert!(
        CHECKER.contains("check-part-logic.vbs") && CHECKER.contains("ExecuteGlobal"),
        "the checker does not share the decision logic"
    );
    // The historical handoff contract stays (also pinned in build_tests).
    assert!(CHECKER.contains("ownedTitle = WScript.Arguments(3)"));
    assert!(CHECKER.contains("closeTitle = ownedTitle"));
    assert!(
        CHECKER.contains("the active document is not the handed-off owned document"),
        "the checker does not verify the handoff before checking"
    );
    // The independent test surface drives the same logic bytes.
    assert!(
        LOGIC_TESTS.contains("ExecuteGlobal logicText") && LOGIC_TESTS.contains("MassLines("),
        "the logic test driver does not drive the shared decision logic"
    );
}

// ---- the harness mass gate ----

#[test]
fn the_harness_requires_mass_pass_except_for_the_declared_skip() {
    assert!(
        HARNESS.contains("check:mass_vs_reference=pass"),
        "the harness does not require the comparison to pass"
    );
    assert!(
        HARNESS.contains("volume_compare=skip"),
        "the harness does not gate the placeholder on the expect declaration"
    );
    assert!(
        HARNESS.contains("$massEntry.Count -eq 1"),
        "the harness does not require the entry exactly once"
    );
    // The committed-corpus provenance requirements are unchanged.
    for pin in ["is not a committed file", "differs from HEAD", "sha256"] {
        assert!(
            HARNESS.contains(pin),
            "the harness lost its provenance gate: {pin}"
        );
    }
}

// ---- the real-part runner's provenance gates ----

#[test]
fn the_real_runner_implements_the_owner_approved_exception() {
    // Every input pinned; anything not pinned is refused.
    for pin in [
        "refusing: the manifest does not pin ",
        "does not match the manifest pin ",
        "Read-RealManifest",
        "Test-RealInputPins",
        "Test-RealToolingPins",
    ] {
        assert!(
            RUNNER.contains(pin),
            "the runner lost a pinning gate: {pin}"
        );
    }
    // The hard rule: real-part artifacts stay untracked and under
    // uploads/<design>/; everything written goes under runs/<design>/.
    for pin in [
        "real-part artifacts must stay untracked",
        "is not inside uploads/",
        "non-relative path",
    ] {
        assert!(
            RUNNER.contains(pin),
            "the runner lost an untracked/confinement gate: {pin}"
        );
    }
    assert!(
        RUNNER.contains("Join-Path (Join-Path $Root \"runs\") $DesignName"),
        "the runner writes outside runs/<design>/"
    );
    // The approval gate: first run needs the owner's approval of the exact
    // macro sha; the same hash proceeds; a changed macro re-approvals.
    for pin in [
        "owner's explicit approval of the exact macro sha256",
        "the macro changed since approval",
        "approved_macro_sha256",
    ] {
        assert!(
            RUNNER.contains(pin),
            "the runner lost an approval gate: {pin}"
        );
    }
    // Real-design acceptance requires the comparison to pass, always
    // comparing (no placeholder).
    let acc = &RUNNER[RUNNER
        .find("function Test-RealAcceptance")
        .expect("no acceptance rule")..];
    assert!(
        acc.contains("$name=pass") && acc.contains("is absent"),
        "the runner's acceptance does not require every verdict to pass"
    );
    // Every run pins what it verified in the untracked run manifest.
    // Finding 5: the EXECUTED bytes are pinned - the files at
    // $PSScriptRoot, this runner included - and consumed fallback sidecars
    // are inputs like any other.
    for pin in [
        "\"tools/seat-run-real.ps1\"",
        "ScriptDir",
        "executed bytes",
        "could not be established",
        "fallback-step-",
        "a consumed fallback sidecar is not pinned",
        "Test-RealZoneUntracked",
    ] {
        assert!(
            RUNNER.contains(pin),
            "the runner lost an execution-path gate: {pin}"
        );
    }
    // Finding 6: a UNIQUE record per run, written before the invocation
    // and patched with its outcome - earlier records are never touched.
    for pin in [
        "New-RunRecord",
        "Complete-RunRecord",
        "run_id",
        "outcome = \"started\"",
    ] {
        assert!(
            RUNNER.contains(pin),
            "the runner lost its durable record: {pin}"
        );
    }
    assert!(
        RUNNER.contains("Join-Path $RunsDir \"runs\""),
        "the run records are not per-run files under runs/<design>/runs/"
    );
    // Finding 7: the runtime report must be complete AND successful with
    // zero failed steps, and acceptance requires the full verdict set.
    for pin in [
        "the runtime summary is not ok",
        "the runtime report carries failed steps",
        "truncated runtime report",
        "entry is not unique",
    ] {
        assert!(
            RUNNER.contains(pin),
            "the runner lost a report/acceptance gate: {pin}"
        );
    }
    let acc = &RUNNER[RUNNER
        .find("function Test-RealAcceptance")
        .expect("no acceptance rule")..];
    for verdict in [
        "check:mass",
        "check:mass_vs_reference",
        "check:step_reference",
        "check:bodies",
        "check:placement",
        "check:ref_close",
    ] {
        assert!(acc.contains(verdict), "the acceptance set lost {verdict}");
    }
    // The provenance tests cover the gates they claim to.
    for pin in [
        "a first run without owner approval is refused",
        "a changed macro needs re-approval",
        "an unpinned input is refused",
        "a tracked artifact is refused",
        "mass_vs_reference=skipped is refused",
        "unpinned tooling is refused",
        "a missing companion verdict is refused",
        "a duplicate verdict is refused",
        "a failed step refuses the run",
        "an unestablishable untracked check fails closed",
        "a tracked file in the output zone is refused",
        "an unpinned consumed sidecar is refused",
        "changed executed bytes are refused",
        "two runs get distinct record files",
        "the later run never overwrites earlier evidence",
    ] {
        assert!(
            RUNNER_TESTS.contains(pin),
            "the runner tests lost a case: {pin}"
        );
    }
}

// ---- the tracking rule (git ls-files) ----

#[test]
fn real_part_artifacts_stay_untracked() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out = Command::new("git")
        .args(["ls-files"])
        .current_dir(root)
        .output()
        .expect("git ls-files must run");
    assert!(out.status.success(), "git ls-files failed");
    let tracked = String::from_utf8_lossy(&out.stdout);
    for line in tracked.lines() {
        assert!(
            !line.starts_with("uploads/") && !line.starts_with("runs/"),
            "a real-part artifact is tracked: {line}"
        );
    }
    let gitignore = std::fs::read_to_string(format!("{root}/.gitignore")).expect(".gitignore");
    for zone in ["uploads/", "runs/"] {
        assert!(
            gitignore.lines().any(|l| l.trim() == zone),
            ".gitignore does not cover {zone}"
        );
    }
}

// ---- the solid=none contract (a no-solid corpus item verifies absence
// two-way; a solid-expected item keeps the fail-closed data gate) ----

fn solid_none_verdict(rebuilt: Option<f64>) -> (&'static str, &'static str) {
    // mirrors MassSkipLines' noSolid branch: (solid, mass) verdicts.
    match rebuilt {
        Some(v) if v > 0.0 => ("fail", "skipped"),
        Some(_) => ("pass", "skipped"),
        None => ("fail", "fail"),
    }
}

#[test]
fn solid_none_is_verified_two_way() {
    // A clean zero volume passes; an unexpected solid fails; an unreadable
    // read cannot prove absence and fails closed.
    assert_eq!(solid_none_verdict(Some(0.0)), ("pass", "skipped"));
    assert_eq!(solid_none_verdict(Some(-1.0)), ("pass", "skipped"));
    assert_eq!(solid_none_verdict(Some(0.0004)), ("fail", "skipped"));
    assert_eq!(solid_none_verdict(None), ("fail", "fail"));
    // The corpus items that build no solid declare it.
    for item in ["frame-omission", "frame-sign-chain", "stop-rule-degrade"] {
        let expect = std::fs::read_to_string(format!("corpus/{item}/expect.txt")).unwrap();
        assert!(
            expect.contains("solid=none"),
            "{item} lost its solid=none declaration"
        );
    }
}
