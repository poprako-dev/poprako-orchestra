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
