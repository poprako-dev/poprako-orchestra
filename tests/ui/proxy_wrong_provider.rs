use poprako_orchestra::{Oper, Run, drive, proxy};

#[derive(Oper)]
#[oper(output = ())]
struct First;

#[derive(Oper)]
#[oper(output = ())]
struct Second;

#[drive(error = (), proxy = PairProxy, run(First, Second))]
trait PairDrive {}

struct Partial;

impl Run<First> for Partial {
    type Error = ();

    async fn run(&self, _oper: &First) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct Complete;

impl Run<First> for Complete {
    type Error = ();

    async fn run(&self, _oper: &First) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Run<Second> for Complete {
    type Error = ();

    async fn run(&self, _oper: &Second) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn require_pair<P: PairProxy>(_proxy: &P) {}

fn main() {
    let partial = &Partial;
    let complete = &Complete;
    let proxy = proxy! {
        run => partial as PairProxy, complete as PairProxy;
    };
    require_pair(&proxy);
}
