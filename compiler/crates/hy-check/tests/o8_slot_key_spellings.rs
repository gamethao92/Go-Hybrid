//! Issue #2226: rule O8 (slot-loan uniqueness, HYB-E3102) compared index
//! keys by their RAW token, so `m[10]` and `m[0xA]` — the same runtime slot
//! — were treated as provably distinct and the second aliasing access was
//! accepted. `hy-emit` lowers each slot loan to copy-out / store-back, so
//! the later store-back silently dropped the first loan's mutation.
//! `lit_index_key` now canonicalizes integer-valued literals through
//! `const_int_value`.

use hy_check::{check_file, CheckResult, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

const MAP_PRELUDE: &str = r#"
struct acct {
    bal int
}

fn main() {
"#;

fn check_src(body: &str) -> CheckResult {
    let src = format!("{}{}\n    println(m[10].bal)\n}}\n", MAP_PRELUDE, body);
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

fn rejects(body: &str) {
    let r = check_src(body);
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3102"),
        "expected HYB-E3102 for `{}`, got {cs:?}",
        body.trim(),
    );
    assert_eq!(r.verdict, Verdict::Reject, "verdict for `{}`", body.trim());
}

fn accepts(body: &str) {
    let r = check_src(body);
    assert_eq!(
        r.verdict,
        Verdict::Accept,
        "`{}` is O8-legal and must be accepted, got {:?}",
        body.trim(),
        codes(&r)
    );
}

// ---- second loan on the same slot, spelled differently ----

#[test]
fn hex_key_aliases_decimal_loan() {
    rejects("    m := map[int]acct{10: acct{bal: 1}}\n    p := &m[10]\n    q := &m[0xA]\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

#[test]
fn octal_key_aliases_decimal_loan() {
    rejects("    m := map[int]acct{10: acct{bal: 1}}\n    p := &m[10]\n    q := &m[0o12]\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

#[test]
fn binary_key_aliases_decimal_loan() {
    rejects("    m := map[int]acct{10: acct{bal: 1}}\n    p := &m[10]\n    q := &m[0b1010]\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

#[test]
fn underscore_key_aliases_decimal_loan() {
    rejects("    m := map[int]acct{10: acct{bal: 1}}\n    p := &m[10]\n    q := &m[1_0]\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

#[test]
fn rune_key_aliases_decimal_loan() {
    // '\n' is 10.
    rejects("    m := map[int]acct{10: acct{bal: 1}}\n    p := &m[10]\n    q := &m['\\n']\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

#[test]
fn legacy_octal_key_aliases_decimal_loan() {
    // `012` is legacy-octal 10.
    rejects("    m := map[int]acct{10: acct{bal: 1}}\n    p := &m[012]\n    q := &m[10]\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

// ---- direct field store inside a live loan window ----

#[test]
fn field_store_hex_key_aliases_slice_loan() {
    let src = "struct acct {\n    bal int\n}\n\nfn main() {\n    ns := []acct{acct{bal: 1}}\n    ns = append(ns, acct{bal: 2})\n    p := &ns[1]\n    ns[0x1].bal = 50\n    p.bal = p.bal + 1\n    println(ns[1].bal)\n}\n";
    let ast = parse_file("inline.hy", src).expect("must parse");
    let r = check_file(&ast);
    assert!(
        codes(&r).iter().any(|c| c == "HYB-E3102"),
        "expected HYB-E3102, got {:?}",
        codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn slice_loan_rune_key_aliases() {
    // ns[10] spelled as the rune '\n' (10) in a 11-element slice.
    let src = "struct acct {\n    bal int\n}\n\nfn main() {\n    ns := []acct{acct{bal: 1}}\n    for i in 0..10 {\n        ns = append(ns, acct{bal: i})\n    }\n    p := &ns[10]\n    q := &ns['\\n']\n    q.bal = 50\n    p.bal = p.bal + 1\n    println(ns[10].bal)\n}\n";
    let ast = parse_file("inline.hy", src).expect("must parse");
    let r = check_file(&ast);
    assert!(
        codes(&r).iter().any(|c| c == "HYB-E3102"),
        "expected HYB-E3102, got {:?}",
        codes(&r)
    );
}

// ---- controls: genuinely distinct slots stay accepted, same spellings too ----

#[test]
fn control_distinct_keys_accepted() {
    accepts("    m := map[int]acct{10: acct{bal: 1}, 11: acct{bal: 2}}\n    p := &m[10]\n    q := &m[0xB]\n    q.bal = 50\n    p.bal = p.bal + 1\n");
}

#[test]
fn control_ident_key_still_conservative() {
    // Non-literal keys are unknown: the pre-existing conservative aliasing
    // report must survive the canonicalization.
    let src = "struct acct {\n    bal int\n}\n\nfn main() {\n    m := map[int]acct{10: acct{bal: 1}}\n    k := 0xA\n    p := &m[10]\n    q := &m[k]\n    q.bal = 50\n    p.bal = p.bal + 1\n    println(m[10].bal)\n}\n";
    let ast = parse_file("inline.hy", src).expect("must parse");
    let r = check_file(&ast);
    assert!(
        codes(&r).iter().any(|c| c == "HYB-E3102"),
        "expected HYB-E3102 for an unknown key, got {:?}",
        codes(&r)
    );
}

#[test]
fn control_string_keys_unaffected() {
    let src = "struct acct {\n    bal int\n}\n\nfn main() {\n    m := map[string]acct{\"a\": acct{bal: 1}, \"b\": acct{bal: 2}}\n    p := &m[\"a\"]\n    q := &m[\"b\"]\n    q.bal = 50\n    p.bal = p.bal + 1\n    println(m[\"a\"].bal)\n}\n";
    let ast = parse_file("inline.hy", src).expect("must parse");
    let r = check_file(&ast);
    assert_eq!(r.verdict, Verdict::Accept, "{:?}", codes(&r));
}

#[test]
fn control_string_key_vs_int_digit_string() {
    // A string key `"10"` and the integer key 10 are different slots in a
    // map with string keys; the canonicalized `int:10` form must not collide
    // with a raw string rendering.
    let src = "struct acct {\n    bal int\n}\n\nfn main() {\n    m := map[string]acct{\"10\": acct{bal: 1}, \"20\": acct{bal: 2}}\n    p := &m[\"10\"]\n    q := &m[\"20\"]\n    q.bal = 50\n    p.bal = p.bal + 1\n    println(m[\"10\"].bal)\n}\n";
    let ast = parse_file("inline.hy", src).expect("must parse");
    let r = check_file(&ast);
    assert_eq!(r.verdict, Verdict::Accept, "{:?}", codes(&r));
}
