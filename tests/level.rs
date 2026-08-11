use poprako_orchestra::nucl::{Nucl, NuclError};
use poprako_orchestra::{AtLeast, Level, Oper, Proxy, Run, Scope, Step};

struct RepeatableRead;
impl Level for RepeatableRead {}

struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

struct Context;
impl Scope for Context {
    type Level = Serializable;
}

struct Read;
impl Oper for Read {
    type Output = ();
    type Level = RepeatableRead;
}

struct Repo;
impl Run<Read> for Repo {
    type Level = Serializable;
    type Error = ();

    async fn run(&self, _oper: &Read) -> Result<(), ()> {
        Ok(())
    }
}

impl Step<Read, Context> for Repo {
    type Error = ();

    async fn step(&self, _context: &mut Context, _oper: &Read) -> Result<(), ()> {
        Ok(())
    }
}

struct Backend;
impl Nucl for Backend {
    type Level = Serializable;
    type Error = ();
    type Context = Context;

    async fn coord<F, T, E>(&self, _f: F) -> Result<T, NuclError<(), E>>
    where
        F: for<'cx> AsyncFnOnce(&'cx mut Context) -> Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        panic!()
    }
}

fn assert_proxy<P>(_proxy: &P)
where
    P: Proxy<Read, Level = Serializable, Error = ()>,
{
}

#[test]
fn stronger_level_satisfies_every_execution_path() {
    fn assert_nucl<N: Nucl<Level = Serializable, Context = Context>>() {}
    assert_nucl::<Backend>();

    let repo = &Repo;
    let run_proxy = poprako_orchestra::run_proxy! {
        repo => Read;
    };
    assert_proxy(&run_proxy);
    drop(run_proxy);

    let mut context = Context;
    let step_proxy = poprako_orchestra::step_proxy! {
        &mut context;
        repo => Read;
    };
    assert_proxy(&step_proxy);
    drop(step_proxy);
}
