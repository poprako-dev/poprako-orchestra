use poprako_orchestra::{Level, Oper, Scope, Step};

struct Weak;
impl Level for Weak {}

struct Strong;
impl Level for Strong {}

struct Context;
impl Scope for Context {
    type Level = Weak;
}

struct StrongOper;
impl Oper for StrongOper {
    type Output = ();
}

struct Repo;
impl Step<StrongOper, Context> for Repo {
    type Level = Strong;
    type Error = ();

    async fn step(&self, _context: &mut Context, _oper: &StrongOper) -> Result<(), ()> {
        Ok(())
    }
}

fn main() {
    let repo = Repo;
    let mut context = Context;
    let oper = StrongOper;
    let _ = repo.step(&mut context, &oper);
}
