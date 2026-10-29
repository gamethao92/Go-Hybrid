//! Regression for issue #2102: builtin argument validation (HYB-E2005) was
//! never mirrored onto array-length expressions. `check_array_len` mirrored
//! the constant zero-divisor predicate (#758) but not
//! `check_expr_builtin_args`, so `var a [len(1)]int`, `type x = [len()]int`
//! or `struct s { a [min()]int }` passed `hyb check` with 0 diagnostics and
//! the emitted Go failed `go build` (`invalid argument ... for built-in len`,
//! `not enough arguments in call to len`).

use hy_check::{check_file, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let path = "inline.hy";
    let ast = parse_file(path, src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {:?}", diags));
    check_file(&ast)
}

fn error_codes(r: &hy_check::CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

fn assert_has(r: &hy_check::CheckResult, code: &str) {
    let cs = error_codes(r);
    assert!(
        cs.iter().any(|c| c == code),
        "expected {}, got {:?}",
        code,
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

fn assert_lacks(r: &hy_check::CheckResult, code: &str) {
    let cs = error_codes(r);
    assert!(
        !cs.iter().any(|c| c == code),
        "did not expect {}, got {:?}",
        code,
        cs
    );
}

#[test]
fn len_of_number_literal_in_array_len_rejected() {
    // `invalid argument: 1 (untyped int constant) for built-in len`
    let r = check("fn main() {\n    var a [len(1)]int\n    _ = a\n}\n");
    assert_has(&r, "HYB-E2005");
}

#[test]
fn cap_of_untyped_string_const_in_array_len_rejected() {
    // Go allows `len` of a string constant but not `cap`
    // (`invalid argument: hi (untyped string constant) for built-in cap`).
    let r = check(
        "const hi string = \"hello\"\n\
         fn main() {\n    b := [cap(hi) + 1]int{0}\n    _ = b\n}\n",
    );
    assert_has(&r, "HYB-E2005");
}

#[test]
fn zero_arity_len_in_type_alias_rejected() {
    // `not enough arguments in call to len`
    let r = check("type long_arr = [len()]int\n");
    assert_has(&r, "HYB-E2005");
}

#[test]
fn zero_arity_min_in_struct_field_array_len_rejected() {
    let r = check("struct boxed {\n    a [min()]int\n}\n");
    assert_has(&r, "HYB-E2005");
}

#[test]
fn len_of_string_const_array_len_still_accepted() {
    // Control: `len(hi)` IS a valid constant length in Go — must not flag.
    let r = check(
        "const hi string = \"hello\"\n\
         fn main() {\n    var a [len(hi)]int\n    _ = a\n}\n",
    );
    assert_lacks(&r, "HYB-E2005");
}

#[test]
fn len_of_local_string_array_len_still_accepted() {
    // Control: value-position builtins keep passing through array lengths
    // where Go does too (a non-constant length fails the pre-existing
    // HYB-E6004 path, not E2005 — no builtin-args error either way).
    let r = check("fn main() {\n    s := \"hello\"\n    var a [3]int\n    _ = s\n    _ = a\n}\n");
    assert_lacks(&r, "HYB-E2005");
}
