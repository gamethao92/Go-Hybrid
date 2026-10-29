//! Regression for issue #2119: `ty_of_expr` / `multi_value_tys` omitted
//! `Expr::Receive`, `Expr::Slice`, and alias resolution on aliased `make`
//! targets, so bindings like `v := <-ch` kept `Binding.ty = None` and the
//! passes keyed on it (HYB-E6002 assign overflow, HYB-E2009 range bounds,
//! HYB-E2002 fn-value arity) silently skipped them — accepted programs
//! failed `go build`.

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
fn reassign_receive_binding_overflows_int8() {
    let r = check(
        "fn run(ch chan int8) {\n\
         \x20   v := <-ch\n\
         \x20   v = 300\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_has(&r, "HYB-E6002");
}

#[test]
fn reassign_comma_ok_receive_binding_overflows_int8() {
    let r = check(
        "fn run(ch chan int8) {\n\
         \x20   v, ok := <-ch\n\
         \x20   v = 300\n\
         \x20   _ = ok\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_has(&r, "HYB-E6002");
}

#[test]
fn reassign_from_aliased_make_chan_overflows_uint8() {
    let r = check(
        "type byte_chan = chan uint8\n\
         fn run() {\n\
         \x20   ch := make(byte_chan, 1)\n\
         \x20   v := <-ch\n\
         \x20   v = 300\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_has(&r, "HYB-E6002");
}

#[test]
fn reassign_from_aliased_map_comma_ok_overflows_uint8() {
    let r = check(
        "type byte_map = map[string]uint8\n\
         fn run(m byte_map) {\n\
         \x20   v, ok := m[\"k\"]\n\
         \x20   v = 300\n\
         \x20   _ = ok\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_has(&r, "HYB-E6002");
}

#[test]
fn receive_binding_fixes_range_bound_overflow() {
    let r = check(
        "fn run(ch chan int8) {\n\
         \x20   v := <-ch\n\
         \x20   for i in v..300 {\n\
         \x20   \x20   _ = i\n\
         \x20   }\n\
         }\n",
    );
    assert_has(&r, "HYB-E2009");
}

#[test]
fn range_over_received_send_only_channel_rejected() {
    let r = check(
        "fn run(outer chan chan<- int) {\n\
         \x20   inner := <-outer\n\
         \x20   for x in inner {\n\
         \x20   \x20   _ = x\n\
         \x20   }\n\
         }\n",
    );
    assert_has(&r, "HYB-E2009");
}

#[test]
fn received_multi_return_fn_single_bind_rejected() {
    let r = check(
        "fn run(ch chan fn(a int, b int) int int) {\n\
         \x20   cb := <-ch\n\
         \x20   p := cb(3, 9)\n\
         \x20   _ = p\n\
         }\n",
    );
    assert_has(&r, "HYB-E2002");
}

#[test]
fn slice_binding_element_overflow_after_reassign() {
    let r = check(
        "fn run(xs []int8) {\n\
         \x20   sub := xs[0:1]\n\
         \x20   v := sub[0]\n\
         \x20   v = 300\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_has(&r, "HYB-E6002");
}

#[test]
fn pointer_to_array_slice_binding_element_overflow() {
    let r = check(
        "fn run(pa *[3]int8) {\n\
         \x20   sub := pa[0:2]\n\
         \x20   v := sub[0]\n\
         \x20   v = 300\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_has(&r, "HYB-E6002");
}

#[test]
fn in_range_receive_reassign_stays_clean() {
    let r = check(
        "fn run(ch chan int8) {\n\
         \x20   v := <-ch\n\
         \x20   v = 7\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_lacks(&r, "HYB-E6002");
    assert_lacks(&r, "HYB-E2009");
}

#[test]
fn in_range_slice_and_comma_ok_stays_clean() {
    let r = check(
        "type byte_map = map[string]int8\n\
         fn run(m byte_map, xs []int8) {\n\
         \x20   sub := xs[0:1]\n\
         \x20   v, ok := m[\"k\"]\n\
         \x20   v = sub[0]\n\
         \x20   _ = ok\n\
         \x20   _ = v\n\
         }\n",
    );
    assert_lacks(&r, "HYB-E6002");
    assert_lacks(&r, "HYB-E2002");
}
