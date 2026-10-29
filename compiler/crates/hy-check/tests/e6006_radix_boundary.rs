//! Issue #2229: `lit_below_2_512` (HYB-E6006's string-conversion boundary)
//! compared the RAW token string against the decimal digits of 2^512, so
//! radix spellings were decided by token length instead of value:
//! `string(0x1 + 128 hex zeros)` (= 2^512, Go rejects with constant
//! overflow) passed the checker, and `string(0b1 + 155 zeros)` (= 2^155,
//! Go builds) was falsely rejected. Legacy octal (`0` + 172 octal digits
//! = 2^512) had the same raw-length defect.

use hy_check::{check_file, CheckResult, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

fn check(src: &str) -> CheckResult {
    let ast = parse_file("inline.hy", src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {:?}", diags));
    check_file(&ast)
}

fn error_codes(r: &CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

fn fn_body(lit: &str) -> String {
    format!("fn main() {{\n    s := string({})\n    fmt.println(\"{{s}}\")\n}}\n", lit)
}

fn rejects(lit: &str) {
    let src = fn_body(lit);
    let r = check(&src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6006"),
        "expected HYB-E6006 for `{}` (>= 2^512), got {:?}",
        lit,
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject, "verdict for `{}`", lit);
}

fn accepts(lit: &str) {
    let src = fn_body(lit);
    let r = check(&src);
    assert_eq!(
        r.verdict,
        Verdict::Accept,
        "`{}` is < 2^512 and must be accepted, got {:?}",
        lit,
        error_codes(&r)
    );
}

// ---- false accepts on main: radix spellings of values >= 2^512 ----

#[test]
fn hex_at_boundary_rejected() {
    // 0x1 followed by 128 zeros = 16^128 = 2^512.
    rejects(&format!("0x1{}", "0".repeat(128)));
}

#[test]
fn hex_above_boundary_rejected() {
    rejects(&format!("0x2{}", "0".repeat(128)));
}

#[test]
fn binary_at_boundary_rejected() {
    // 0b1 followed by 512 zeros = 2^512.
    rejects(&format!("0b1{}", "0".repeat(512)));
}

#[test]
fn octal_prefixed_at_boundary_rejected() {
    // 2^512 = 0o1 followed by 171 zeros (172 raw digits, unlike decimal's 155).
    rejects(&format!("0o1{}", "0".repeat(171)));
}

#[test]
fn legacy_octal_at_boundary_rejected() {
    // Leading-0 legacy octal for 2^512: octal digits 1 + 171 zeros, spelled
    // with the implicit leading 0 (an all-zero string is just 0).
    rejects(&format!("01{}", "0".repeat(171)));
}

// ---- false rejects on main: radix spellings of values < 2^512 whose raw
// token is longer than the 155-digit decimal boundary string ----

#[test]
fn binary_below_boundary_accepted() {
    // 2^155 < 2^512, yet its raw token (157 chars) defeated the length
    // comparison on main.
    accepts(&format!("0b1{}", "0".repeat(155)));
}

#[test]
fn hex_below_boundary_accepted() {
    // 128 f's = 2^512 - 1: the max-magnitude hex spelling (exactly 512 bits).
    accepts(&format!("0x{}", "f".repeat(128)));
}

#[test]
fn hex_all_f_over_boundary_rejected() {
    // 0xff..f (130 f's) = 2^520 - 1 > 2^512 — main decided this from the
    // raw token against the decimal boundary string instead of the value.
    rejects(&format!("0x{}", "f".repeat(129)));
}

#[test]
fn octal_below_boundary_accepted() {
    // 0o3 + 170 sevens = 2^512 - 1 exactly (3 + 170*3 = 512 bits).
    accepts(&format!("0o3{}", "7".repeat(170))); // 2^512 - 1
}

// ---- controls: the decimal boundary must keep its exact behaviour ----

#[test]
fn decimal_at_boundary_still_rejected() {
    rejects(TWO_POW_512);
}

#[test]
fn decimal_below_boundary_still_accepted() {
    // 2^512 - 1
    accepts("13407807929942597099574024998205846127479365820592393377723561443721764030073546976801874298166903427690031858186486050853753882811946569946433649006084095");
}

const TWO_POW_512: &str = "13407807929942597099574024998205846127479365820592393377723561443721764030073546976801874298166903427690031858186486050853753882811946569946433649006084096";
