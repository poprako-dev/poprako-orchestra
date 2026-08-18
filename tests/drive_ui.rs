#![cfg(feature = "macro")]

#[test]
fn drive_accepts_complex_bounds() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/drive_complex.rs");
}

#[test]
fn drive_propagates_per_step_levels() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/drive_level.rs");
}
