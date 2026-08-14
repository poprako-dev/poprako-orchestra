use poprako_orchestra::{Level, Oper, Context, Step};

struct Weak;
impl Level for Weak {}

struct Strong;
impl Level for Strong {}

struct Cx;
impl Context for Cx {
    type Level = Weak;
}

struct StrongOper;
impl Oper for StrongOper {
    type Output = ();
}

struct Repo;
impl Step<StrongOper, Cx> for Repo {
    type Level = Strong;
    type Error = ();

    async fn step(&self, _context: &mut Cx, _oper: &StrongOper) -> Result<(), ()> {
        Ok(())
    }
}

fn main() {
    let repo = Repo;
    let mut context = Cx;
    let oper = StrongOper;
    let _ = repo.step(&mut context, &oper);
}
