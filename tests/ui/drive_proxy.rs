use std::marker::PhantomData;

use poprako_orchestra::{
    AtLeast, Level, Oper, Proxy, Run, Context, Step, drive,
};

struct Transactional;

impl Level for Transactional {}

#[derive(Oper)]
#[oper(output = T)]
struct FindUser<T, const N: usize> {
    _payload: PhantomData<T>,
}

#[derive(Oper)]
#[oper(output = T)]
struct UpdateUser<'a, 'b, T, const N: usize> {
    marker: PhantomData<(&'a (), &'b (), T)>,
}

struct Error;

// One proxy trait merges the run and step operation sets; `C` is absent from
// its generics.
#[drive(
    context = C,
    error = Error,
    proxy = UserRepoProxy,
    run(FindUser<T, N>),
    step(for<'a, 'b> UpdateUser<'a, 'b, T, N>),
)]
trait UserRepo<C, T, const N: usize>: Send
where
    T: Send,
{
}

struct Repo;

impl<T, const N: usize> Run<FindUser<T, N>> for Repo
where
    T: Sync,
{
    type Error = Error;

    async fn run(&self, _oper: &FindUser<T, N>) -> Result<T, Self::Error> {
        panic!()
    }
}

impl<'a, 'b, C, T, const N: usize> Step<UpdateUser<'a, 'b, T, N>, C> for Repo
where
    C: Context + Send,
    C::Level: AtLeast<Transactional>,
    T: Sync,
{
    type Level = Transactional;
    type Error = Error;

    async fn step(
        &self,
        _context: &mut C,
        _oper: &UpdateUser<'a, 'b, T, N>,
    ) -> Result<T, Self::Error> {
        panic!()
    }
}

fn assert_user_repo<C, T, const N: usize>()
where
    C: Context + Send,
    C::Level: AtLeast<Transactional>,
    T: Send + Sync,
    Repo: UserRepo<C, T, N>,
{
}

struct ProxyImpl;

impl<T, const N: usize> Proxy<FindUser<T, N>> for ProxyImpl
where
    T: Sync,
{
    type Error = Error;

    async fn exec(&mut self, _oper: &FindUser<T, N>) -> Result<T, Self::Error> {
        panic!()
    }
}

impl<'a, 'b, T, const N: usize> Proxy<UpdateUser<'a, 'b, T, N>> for ProxyImpl
where
    T: Sync,
{
    type Error = Error;

    async fn exec(
        &mut self,
        _oper: &UpdateUser<'a, 'b, T, N>,
    ) -> Result<T, Self::Error> {
        panic!()
    }
}

fn assert_user_proxy<T, const N: usize>()
where
    T: Send + Sync,
    ProxyImpl: UserRepoProxy<T, N>,
{
}

struct Cx;

impl Context for Cx {
    type Level = Transactional;
}

fn main() {
    assert_user_repo::<Cx, String, 1>();
    assert_user_proxy::<String, 1>();
}
