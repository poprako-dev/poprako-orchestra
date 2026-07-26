#![cfg(feature = "macro")]

#[test]
fn drive_accepts_complex_bounds() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/drive_complex.rs");
}
