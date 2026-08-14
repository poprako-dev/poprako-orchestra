#![cfg(feature = "macro")]

use std::marker::PhantomData;

use poprako_orchestra::{Context, Level, Oper, Run, Step, drive};

struct Transactional;

impl Level for Transactional {}

#[derive(Oper)]
#[oper(output = Option<String>)]
struct ExistAvatar<'a> {
    id: &'a str,
}

#[derive(Oper)]
#[oper(output = T)]
struct GenericOper<'a, T>
where
    T: std::fmt::Debug + 'a,
{
    payload: &'a T,
}

#[derive(Oper)]
#[oper(output = T)]
struct FindUser<T> {
    _payload: T,
}

#[derive(Oper)]
#[oper(output = T)]
struct UpdateUser<'a, 'b, T> {
    marker: PhantomData<(&'a (), &'b (), T)>,
}

#[drive(
    context = C,
    error = TestError,
    run(for<'a> ExistAvatar<'a>, FindUser<T>),
    step(for<'a, 'b> UpdateUser<'a, 'b, T>),
)]
trait UserRepo<C, T>
where
    T: std::fmt::Debug + Send,
{
}

struct TestError;

struct Repo;

impl Run<ExistAvatar<'_>> for Repo {
    type Error = TestError;

    async fn run(&self, _oper: &ExistAvatar<'_>) -> Result<Option<String>, Self::Error> {
        Ok(None)
    }
}

impl<T> Run<FindUser<T>> for Repo
where
    T: Sync,
{
    type Error = TestError;

    async fn run(&self, _oper: &FindUser<T>) -> Result<T, Self::Error> {
        panic!()
    }
}

impl<'a, 'b, C, T> Step<UpdateUser<'a, 'b, T>, C> for Repo
where
    C: Context + Send,
    C::Level: poprako_orchestra::AtLeast<Transactional>,
    T: Sync,
{
    type Level = Transactional;
    type Error = TestError;

    async fn step(
        &self,
        _context: &mut C,
        _oper: &UpdateUser<'a, 'b, T>,
    ) -> Result<T, Self::Error> {
        panic!()
    }
}

fn assert_output<O: Oper<Output = Output>, Output>() {}

#[test]
fn derives_oper_for_plain_and_generic_structs() {
    assert_output::<ExistAvatar<'_>, Option<String>>();
    assert_output::<GenericOper<'_, String>, String>();

    let oper = ExistAvatar { id: "avatar" };
    assert_eq!(oper.id, "avatar");

    let payload = String::from("payload");
    let oper = GenericOper { payload: &payload };
    assert_eq!(oper.payload, "payload");
}

fn assert_user_repo<C, T>()
where
    T: std::fmt::Debug + Send + Sync,
    C: Context + Send,
    C::Level: poprako_orchestra::AtLeast<Transactional>,
{
    fn assert_impl<C, T, Repo>()
    where
        T: std::fmt::Debug + Send,
        C: Context,
        C::Level: poprako_orchestra::AtLeast<Transactional>,
        Repo: UserRepo<C, T>,
    {
    }

    assert_impl::<C, T, Repo>();
}

struct Cx;

impl Context for Cx {
    type Level = Transactional;
}

#[test]
fn derives_aggregate_repo_trait() {
    assert_user_repo::<Cx, String>();
}
