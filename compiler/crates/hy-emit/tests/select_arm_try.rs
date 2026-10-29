//! Regression for issue #1902: `?` inside a `select` arm HEADER
//! (`v := <-get_chan()? => {..}`, `ch <- next_val()? => {..}`) passed
//! hy-check but broke emission — `stmt_has_try` ignored arm headers (so the
//! E6 `hyErr` wrapper was omitted) and `emit_stmt`/`Stmt::Select` emitted
//! the case heads without hoisting, hitting `unsupported: ? outside a
//! binding position`.

use hy_emit::emit_go;
use hy_syntax::parse_file;

fn emit_src(src: &str) -> String {
    let ast = parse_file("test.hy", src).expect("parses");
    emit_go(&ast).expect("emits")
}

const REPRO: &str = "\
fn get_chan() (chan int, error) {
    return make(chan int, 1), nil
}

fn next_val() (int, error) {
    return 7, nil
}

fn main() {
    ch := make(chan int, 1)
    select {
    v := <-get_chan()? => {
        _ = v
    }
    ch <- next_val()? => {}
    default => {}
    }
}
";

#[test]
fn select_recv_arm_try_emits() {
    let go = emit_src(
        "\
fn get_chan() (chan int, error) {
    return make(chan int, 1), nil
}

fn main() {
    select {
    v := <-get_chan()? => {
        _ = v
    }
    default => {}
    }
}
",
    );
    assert!(go.contains("select {"), "got:\n{go}");
    assert!(go.contains("get_chan()"), "got:\n{go}");
    // the hoisted receive-channel call must land BEFORE `select {` (Go
    // evaluates arm expressions in source order before the select blocks)
    assert!(
        go.find("get_chan()").unwrap() < go.find("select {").unwrap(),
        "hoist did not precede select:\n{go}"
    );
}

#[test]
fn select_send_arm_try_emits() {
    let go = emit_src(
        "\
fn next_val() (int, error) {
    return 7, nil
}

fn main() {
    ch := make(chan int, 1)
    select {
    ch <- next_val()? => {}
    default => {}
    }
}
",
    );
    assert!(go.contains("select {"), "got:\n{go}");
    assert!(
        go.find("next_val()").unwrap() < go.find("select {").unwrap(),
        "hoist did not precede select:\n{go}"
    );
}

#[test]
fn select_only_try_in_head_still_gets_e6_wrapper() {
    // Before the fix, `stmt_has_try` saw no `?` (it only walked arm bodies),
    // so `main_needs_wrapper` was false and the hoisted propagation returned
    // through a wrapper that did not exist.
    let go = emit_src(REPRO);
    assert!(go.contains("hyErr"), "E6 wrapper missing:\n{go}");
    assert!(go.contains("select {"), "got:\n{go}");
}
