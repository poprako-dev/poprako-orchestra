#![cfg(feature = "macro")]

use std::marker::PhantomData;

use poprako_orchestra::{
    AtLeast, Level, Oper, Proxy, Run, Scope, Step, drive, run_proxy, step_proxy,
};

struct Transactional;

impl Level for Transactional {}

struct Context;

impl Scope for Context {
    type Level = Transactional;
}

// --- Concrete-context case: one oper list drives the generated proxy trait ---

#[derive(Oper)]
#[oper(output = (), level = Transactional)]
struct EnsureCustomer<'a> {
    customer_id: &'a str,
}

#[derive(Oper)]
#[oper(output = u64, level = Transactional)]
struct CreateOrder<'a> {
    customer_id: &'a str,
    quantity: u32,
}

// `run(...)` and `step(...)` list the same opers; the macro merges and
// deduplicates them into the generated `OrderRepoProxy`.
#[drive(
    context = Context,
    error = String,
    proxy = OrderRepoProxy,
    run(for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>),
    step(for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>),
)]
trait OrderRepo {}

struct Repo;

impl Run<EnsureCustomer<'_>> for Repo {
    type Level = Transactional;
    type Error = String;

    async fn run(&self, oper: &EnsureCustomer<'_>) -> Result<(), Self::Error> {
        if oper.customer_id.is_empty() {
            return Err("customer ID must not be empty".to_owned());
        }

        Ok(())
    }
}

impl Run<CreateOrder<'_>> for Repo {
    type Level = Transactional;
    type Error = String;

    async fn run(&self, oper: &CreateOrder<'_>) -> Result<u64, Self::Error> {
        Ok(oper.customer_id.len() as u64 + oper.quantity as u64)
    }
}

impl Step<EnsureCustomer<'_>, Context> for Repo {
    type Error = String;

    async fn step(
        &self,
        _context: &mut Context,
        oper: &EnsureCustomer<'_>,
    ) -> Result<(), Self::Error> {
        if oper.customer_id.is_empty() {
            return Err("customer ID must not be empty".to_owned());
        }

        Ok(())
    }
}

impl Step<CreateOrder<'_>, Context> for Repo {
    type Error = String;

    async fn step(
        &self,
        _context: &mut Context,
        oper: &CreateOrder<'_>,
    ) -> Result<u64, Self::Error> {
        Ok(oper.customer_id.len() as u64 + oper.quantity as u64)
    }
}

fn require_repo<R: OrderRepo>() {}

fn require_proxy<P: OrderRepoProxy>(_proxy: &P) {}

#[test]
fn generated_proxy_trait_unifies_run_and_step_oper_lists() {
    // The aggregate repo trait and the generated proxy trait are both satisfied.
    require_repo::<Repo>();

    let repo = &Repo;

    let mut run_proxy = run_proxy! {
        repo => for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>;
    };
    require_proxy(&run_proxy);
    drop(run_proxy.exec(&EnsureCustomer { customer_id: "c" }));
    drop(run_proxy.exec(&CreateOrder {
        customer_id: "c",
        quantity: 1,
    }));

    let mut context = Context;
    let context = &mut context;
    let mut step_proxy = step_proxy! {
        context;
        repo => for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>;
    };
    require_proxy(&step_proxy);
    drop(step_proxy.exec(&EnsureCustomer { customer_id: "c" }));
    drop(step_proxy.exec(&CreateOrder {
        customer_id: "c",
        quantity: 1,
    }));
}

// --- Generic-context case: the proxy trait drops the `context` type param ---

#[derive(Oper)]
#[oper(output = T, level = Transactional)]
struct FindUser<T, const N: usize> {
    _payload: PhantomData<T>,
}

#[derive(Oper)]
#[oper(output = T, level = Transactional)]
struct UpdateUser<'a, 'b, T, const N: usize> {
    _marker: PhantomData<(&'a (), &'b (), T)>,
}

struct TestError;

// `UserRepo<C, T, N>` drives both `UserRepoProxy<T, N>` — `C` (the context) is
// not a generic parameter of the generated proxy trait.
#[drive(
    context = C,
    error = TestError,
    proxy = UserRepoProxy,
    run(FindUser<T, N>),
    step(for<'a, 'b> UpdateUser<'a, 'b, T, N>),
)]
trait UserRepo<C, T, const N: usize>
where
    T: Send,
{
}

struct GenericRepo;

impl<T, const N: usize> Run<FindUser<T, N>> for GenericRepo
where
    T: Sync,
{
    type Level = Transactional;
    type Error = TestError;

    async fn run(&self, _oper: &FindUser<T, N>) -> Result<T, Self::Error> {
        panic!()
    }
}

impl<'a, 'b, C, T, const N: usize> Step<UpdateUser<'a, 'b, T, N>, C> for GenericRepo
where
    C: Scope + Send,
    C::Level: AtLeast<Transactional>,
    T: Sync,
{
    type Error = TestError;

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
    GenericRepo: UserRepo<C, T, N>,
{
}

struct DummyProxy;

impl Proxy<FindUser<String, 1>> for DummyProxy {
    type Level = Transactional;
    type Error = TestError;

    async fn exec(&mut self, _oper: &FindUser<String, 1>) -> Result<String, Self::Error> {
        Ok(String::new())
    }
}

impl<'a, 'b> Proxy<UpdateUser<'a, 'b, String, 1>> for DummyProxy {
    type Level = Transactional;
    type Error = TestError;

    async fn exec(
        &mut self,
        _oper: &UpdateUser<'a, 'b, String, 1>,
    ) -> Result<String, Self::Error> {
        Ok(String::new())
    }
}

fn assert_user_proxy<T, const N: usize>()
where
    T: Send + Sync,
    DummyProxy: UserRepoProxy<T, N>,
{
}

#[test]
fn generic_context_proxy_trait_drops_context_param() {
    assert_user_repo::<Context, String, 1>();
    assert_user_proxy::<String, 1>();
}
