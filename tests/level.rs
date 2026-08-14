use poprako_orchestra::nucl::{Nucl, NuclError};
use poprako_orchestra::{AtLeast, Context, Level, Oper, Proxy, Run, Step};

struct RepeatableRead;
impl Level for RepeatableRead {}

struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

struct Cx;
impl Context for Cx {
    type Level = Serializable;
}

struct Read;
impl Oper for Read {
    type Output = ();
}

struct Write;
impl Oper for Write {
    type Output = ();
}

struct Repo;
impl Run<Read> for Repo {
    type Error = ();

    async fn run(&self, _oper: &Read) -> Result<(), ()> {
        Ok(())
    }
}

impl Step<Read, Cx> for Repo {
    type Level = RepeatableRead;
    type Error = ();

    async fn step(&self, _context: &mut Cx, _oper: &Read) -> Result<(), ()> {
        Ok(())
    }
}

impl Step<Write, Cx> for Repo {
    type Level = Serializable;
    type Error = ();

    async fn step(&self, _context: &mut Cx, _oper: &Write) -> Result<(), ()> {
        Ok(())
    }
}

#[cfg(feature = "macro")]
#[poprako_orchestra::drive(
    context = Cx,
    error = (),
    step(Read, Write),
)]
trait MixedLevelDriver {}

#[cfg(feature = "macro")]
fn assert_mixed_level_driver<D: MixedLevelDriver>() {}

struct Backend;
impl Nucl for Backend {
    type Level = Serializable;
    type Error = ();
    type Context = Cx;

    async fn coord<F, T, E>(&self, _f: F) -> Result<T, NuclError<(), E>>
    where
        F: for<'cx> AsyncFnOnce(&'cx mut Cx) -> Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        panic!()
    }
}

fn assert_proxy<P>(_proxy: &P)
where
    P: Proxy<Read, Error = ()>,
{
}

#[test]
fn run_has_no_level_and_steps_can_require_different_levels() {
    fn assert_nucl<N: Nucl<Level = Serializable, Context = Cx>>() {}
    assert_nucl::<Backend>();

    #[cfg(feature = "macro")]
    assert_mixed_level_driver::<Repo>();

    let repo = &Repo;
    {
        let run_proxy = poprako_orchestra::run_proxy! {
            repo => Read;
        };
        assert_proxy(&run_proxy);
    }

    let mut context = Cx;
    {
        let step_proxy = poprako_orchestra::step_proxy! {
            &mut context;
            repo => Read, Write;
        };
        assert_proxy(&step_proxy);
    }
}
