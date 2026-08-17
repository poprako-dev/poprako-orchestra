use poprako_orchestra::{Oper, Run, drive, proxy};

#[derive(Oper)]
#[oper(output = ())]
struct Read;

#[drive(error = (), proxy = ReadProxy, run(Read))]
trait ReadDrive {}

struct Repo;

impl Run<Read> for Repo {
    type Error = ();

    async fn run(&self, _oper: &Read) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn main() {
    let repo = &Repo;
    let _proxy = proxy! {
        priority => run, run;
        run => repo as ReadProxy;
    };
}
