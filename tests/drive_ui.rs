#![cfg(feature = "macro")]

#[test]
fn drive_accepts_complex_bounds() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/drive_complex.rs");
}

#[test]
fn drive_accepts_proxy_generation() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/drive_proxy.rs");
}

#[test]
fn drive_rejects_proxy_without_matching_operations() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/drive_proxy_missing_*.rs");
    tests.compile_fail("tests/ui/drive_proxy_same_name.rs");
}
