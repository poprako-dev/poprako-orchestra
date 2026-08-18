#![cfg(feature = "macro")]

use poprako_orchestra::{Context, Level, Oper, Run, Step, drive};

struct Serializable;
impl Level for Serializable {}

struct Cx;
impl Context for Cx {
    type Level = Serializable;
}

#[derive(Oper)]
#[oper(output = ())]
struct Read;

#[derive(Oper)]
#[oper(output = ())]
struct Write;

#[drive(error = (), run(Read))]
trait ReadRepo {}

#[drive(context = Cx, error = (), step(Write))]
trait WriteRepo {}

struct RunOnly;
impl Run<Read> for RunOnly {
    type Error = ();

    async fn run(&self, _oper: &Read) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct StepOnly;
impl Step<Write, Cx> for StepOnly {
    type Level = Serializable;
    type Error = ();

    async fn step(&self, _context: &mut Cx, _oper: &Write) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn drive_keeps_run_and_step_aggregates_independent() {
    fn assert_run<R: ReadRepo>() {}
    fn assert_step<R: WriteRepo>() {}

    assert_run::<RunOnly>();
    assert_step::<StepOnly>();
}
