#![allow(dead_code)]

use poprako_orchestra::{AtLeast, Context, Level, Oper, Run, Step};

struct RepeatableRead;
impl Level for RepeatableRead {}

struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

struct Cx;
impl Context for Cx {
    type Level = Serializable;
}

struct Read;
impl Oper for Read {
    type Output = ();
}

struct Write;
impl Oper for Write {
    type Output = ();
}

struct Repo;
impl Run<Read> for Repo {
    type Error = ();

    async fn run(&self, _oper: &Read) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Step<Read, Cx> for Repo {
    type Level = RepeatableRead;
    type Error = ();

    async fn step(&self, _context: &mut Cx, _oper: &Read) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Step<Write, Cx> for Repo {
    type Level = Serializable;
    type Error = ();

    async fn step(&self, _context: &mut Cx, _oper: &Write) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn run_has_no_level_and_steps_can_require_different_levels() {
    fn assert_run<R: Run<Read, Error = ()>>() {}
    fn assert_step<R: Step<Read, Cx, Error = ()>>()
    where
        R: LevelGuardForRead,
    {
    }
    assert_run::<Repo>();
    assert_step::<Repo>();
}

trait LevelGuardForRead {}
impl<T> LevelGuardForRead for T where
    T: Step<Read, Cx> + poprako_orchestra::LevelGuard<Serializable, T::Level>
{
}
