use std::marker::PhantomData;

use poprako_orchestra::{AtLeast, Level, Oper, Run, Scope, Step, drive};

struct Transactional;

impl Level for Transactional {}

#[derive(Oper)]
#[oper(output = T)]
struct FindUser<T, const N: usize> {
    _payload: T,
}

#[derive(Oper)]
#[oper(output = T)]
struct UpdateUser<'a, 'b, T, const N: usize> {
    marker: PhantomData<(&'a (), &'b (), T)>,
}

struct Error;

#[drive(
    context = C,
    error = Error,
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
    C: Scope + Send,
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
    C: Scope + Send,
    C::Level: AtLeast<Transactional>,
    T: Send + Sync,
    Repo: UserRepo<C, T, N>,
{
}

struct Context;

impl Scope for Context {
    type Level = Transactional;
}

fn main() {
    assert_user_repo::<Context, String, 1>();
}
