use poprako_orchestra::{Oper, drive};

#[derive(Oper)]
#[oper(output = ())]
struct Write;

#[drive(
    context = C,
    error = (),
    run_proxy = RepoRunProxy,
    step(Write),
)]
trait Repo<C> {}

fn main() {}
