#![cfg(feature = "macro")]

#[test]
fn rejects_invalid_level_contracts() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/level/*.rs");
}
