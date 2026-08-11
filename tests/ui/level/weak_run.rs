use poprako_orchestra::{Level, Oper, Run};

struct Weak;
impl Level for Weak {}

struct Strong;
impl Level for Strong {}

struct StrongOper;
impl Oper for StrongOper {
    type Output = ();
    type Level = Strong;
}

struct Repo;
impl Run<StrongOper> for Repo {
    type Level = Weak;
    type Error = ();

    async fn run(&self, _oper: &StrongOper) -> Result<(), ()> {
        Ok(())
    }
}

fn main() {}
