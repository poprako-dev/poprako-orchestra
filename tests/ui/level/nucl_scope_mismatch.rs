use poprako_orchestra::nucl::{Nucl, NuclError};
use poprako_orchestra::{Level, Context};

struct Weak;
impl Level for Weak {}

struct Strong;
impl Level for Strong {}

struct Cx;
impl Context for Cx {
    type Level = Weak;
}

struct Backend;
impl Nucl for Backend {
    type Level = Strong;
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

fn main() {}
