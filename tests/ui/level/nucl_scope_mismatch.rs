use poprako_orchestra::nucl::{Nucl, NuclError};
use poprako_orchestra::{Level, Scope};

struct Weak;
impl Level for Weak {}

struct Strong;
impl Level for Strong {}

struct Context;
impl Scope for Context {
    type Level = Weak;
}

struct Backend;
impl Nucl for Backend {
    type Level = Strong;
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

fn main() {}
