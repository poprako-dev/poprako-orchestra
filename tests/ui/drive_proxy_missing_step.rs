use poprako_orchestra::{Oper, drive};

#[derive(Oper)]
#[oper(output = ())]
struct Read;

#[drive(
    error = (),
    step_proxy = RepoStepProxy,
    run(Read),
)]
trait Repo {}

fn main() {}
