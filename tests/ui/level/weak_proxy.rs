use poprako_orchestra::{Level, Oper, Proxy};

struct Weak;
impl Level for Weak {}

struct Strong;
impl Level for Strong {}

struct StrongOper;
impl Oper for StrongOper {
    type Output = ();
    type Level = Strong;
}

struct Erased;
impl Proxy<StrongOper> for Erased {
    type Level = Weak;
    type Error = ();

    async fn exec(&mut self, _oper: &StrongOper) -> Result<(), ()> {
        Ok(())
    }
}

fn main() {}
