use poprako_orchestra::{Oper, drive};

#[derive(Oper)]
#[oper(output = ())]
struct Operation;

#[drive(
    context = C,
    error = (),
    run_proxy = RepoProxy,
    step_proxy = RepoProxy,
    run(Operation),
    step(Operation),
)]
trait Repo<C> {}

fn main() {}
