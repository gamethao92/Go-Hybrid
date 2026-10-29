//! Issue #2238: HYB-E3012 (chan-direction-coercion, Tier-0 rule A1c) is
//! bypassed in two composite-literal value-flow positions, so a `chan<- int`
//! sent where a `<-chan int` is required passes `hyb check` and the emitted
//! Go fails `go build` (GUARANTEES.md Row 13 BUILDABLE):
//!
//! 1. array composite literals — `walk_expr_chan_dir` handed the OUTER
//!    `TypeExpr::Array` (how `[N]T{…}` is stored in `Expr::SliceLit.elem`)
//!    to `check_expr_against_ty` instead of the element type `T`, and nested
//!    type-elided literals inside them inherited the same wrong width;
//! 2. `move`-wrapped type-elided literals — `check_expr_against_ty` matched
//!    `unwrap_parens(expr)`, so `move {ch: sc}` never reached the
//!    `StructLit { ty: None }` / `SliceLit { elem: None }` validation.

use hy_check::{check_file, CheckResult, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

const PRELUDE: &str = r#"
struct holder {
  ch <-chan int
}

fn take_holder(h holder) {
  _ = h.ch
}

fn main() {
"#;

fn check_src(body: &str) -> CheckResult {
    let src = format!("{}{}\n}}\n", PRELUDE, body);
    let ast = parse_file("inline.hy", &src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {diags:?}"));
    check_file(&ast)
}

fn codes(r: &CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

fn rejects_3012(body: &str) {
    let r = check_src(body);
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3012"),
        "expected HYB-E3012 for `{}`, got {cs:?}",
        body.trim(),
    );
    assert_eq!(r.verdict, Verdict::Reject, "verdict for `{}`", body.trim());
}

fn accepts(body: &str) {
    let r = check_src(body);
    assert_eq!(
        r.verdict,
        Verdict::Accept,
        "`{}` is direction-legal and must be accepted, got {:?}",
        body.trim(),
        codes(&r)
    );
}

// ---- 1. typed array composite literals ----

#[test]
fn fixed_array_direct_channel_element() {
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  a1 := [1]<-chan int{sc}\n  _ = a1");
}

#[test]
fn ellipsis_array_direct_channel_element() {
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  a2 := [...]<-chan int{sc}\n  _ = a2");
}

#[test]
fn fixed_array_of_both_direction_channel() {
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  a3 := [1]chan int{sc}\n  _ = a3");
}

#[test]
fn nested_elided_struct_inside_array() {
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  a4 := [1]holder{{ch: sc}}\n  _ = a4");
}

#[test]
fn nested_elided_array_inside_array() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  a5 := [1][1]<-chan int{{sc}}\n  _ = a5",
    );
}

#[test]
fn nested_elided_slice_inside_array() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  a6 := [1][]<-chan int{{sc}}\n  _ = a6",
    );
}

// ---- 2. move-wrapped type-elided composite literals ----

#[test]
fn move_elided_struct_in_channel_send() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  ch_h := make(chan holder, 1)\n  ch_h <- move {ch: sc}",
    );
}

#[test]
fn move_elided_struct_in_var_decl() {
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  var h holder = move {ch: sc}\n  _ = h");
}

#[test]
fn move_elided_struct_in_assignment() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  var h holder = {ch: <-make(chan int, 1)}\n  h = move {ch: sc}",
    );
}

#[test]
fn move_elided_struct_as_call_arg() {
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  take_holder(move {ch: sc})");
}

#[test]
fn move_elided_struct_in_task_send() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  task take_holder(move {ch: sc})",
    );
}

#[test]
fn move_elided_struct_in_return_position() {
    // `[]holder{move {ch: sc}}` — a move-wrapped elided literal as a slice
    // literal element (the issue's list includes the array form too).
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  hs := []holder{move {ch: sc}}\n  _ = hs",
    );
}

#[test]
fn move_elided_struct_in_map_value() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  ms := map[string]holder{\"k\": move {ch: sc}}\n  _ = ms",
    );
}

#[test]
fn move_elided_struct_in_option() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  var o option[holder] = some(move {ch: sc})\n  _ = o",
    );
}

#[test]
fn move_elided_array_element() {
    rejects_3012(
        "  var sc chan<- int = make(chan int, 1)\n  a := [1]holder{move {ch: sc}}\n  _ = a",
    );
}

// ---- controls: direction-legal forms keep passing ----

#[test]
fn control_receiving_channel_in_array_ok() {
    accepts("  rc := (<-make(chan chan int, 1))\n  _ = rc");
    accepts("  var c chan int = make(chan int, 1)\n  a := [1]<-chan int{(<-make(chan chan int, 1))}\n  _ = a\n  _ = c");
}

#[test]
fn control_plain_elided_struct_still_checked() {
    // Not move-wrapped: main already rejected this — the fix must not
    // regress it.
    rejects_3012("  var sc chan<- int = make(chan int, 1)\n  var h holder = {ch: sc}\n  _ = h");
}
