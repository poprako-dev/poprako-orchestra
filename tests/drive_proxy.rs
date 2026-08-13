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

// --- Concrete-context case: run and step generate separate proxy traits ---

#[derive(Oper)]
#[oper(output = ())]
struct EnsureCustomer<'a> {
    customer_id: &'a str,
}

#[derive(Oper)]
#[oper(output = u64)]
struct CreateOrder<'a> {
    customer_id: &'a str,
    quantity: u32,
}

#[drive(
    context = Context,
    error = String,
    run_proxy = OrderRepoRunProxy,
    step_proxy = OrderRepoStepProxy,
    run(for<'a> EnsureCustomer<'a>),
    step(for<'a> CreateOrder<'a>),
)]
trait OrderRepo {}

struct Repo;

impl Run<EnsureCustomer<'_>> for Repo {
    type Error = String;

    async fn run(&self, oper: &EnsureCustomer<'_>) -> Result<(), Self::Error> {
        if oper.customer_id.is_empty() {
            return Err("customer ID must not be empty".to_owned());
        }

        Ok(())
    }
}

impl Step<CreateOrder<'_>, Context> for Repo {
    type Level = Transactional;
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

fn require_run_proxy<P: OrderRepoRunProxy>(_proxy: &P) {}

fn require_step_proxy<P: OrderRepoStepProxy>(_proxy: &P) {}

#[test]
fn generated_proxy_traits_preserve_asymmetric_oper_lists() {
    require_repo::<Repo>();

    let repo = &Repo;

    let mut run_proxy = run_proxy! {
        repo => for<'a> EnsureCustomer<'a>;
    };
    require_run_proxy(&run_proxy);
    drop(run_proxy.exec(&EnsureCustomer { customer_id: "c" }));

    let mut context = Context;
    let context = &mut context;
    let mut step_proxy = step_proxy! {
        context;
        repo => for<'a> CreateOrder<'a>;
    };
    require_step_proxy(&step_proxy);
    drop(step_proxy.exec(&CreateOrder {
        customer_id: "c",
        quantity: 1,
    }));
}

// --- Generic-context case: the proxy trait drops the `context` type param ---

#[derive(Oper)]
#[oper(output = T)]
struct FindUser<T, const N: usize> {
    _payload: PhantomData<T>,
}

#[derive(Oper)]
#[oper(output = T)]
struct UpdateUser<'a, 'b, T, const N: usize> {
    _marker: PhantomData<(&'a (), &'b (), T)>,
}

struct TestError;

// Both proxy traits drop `C`, while retaining only their own operation sets.
#[drive(
    context = C,
    error = TestError,
    run_proxy = UserRepoRunProxy,
    step_proxy = UserRepoStepProxy,
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
    type Level = Transactional;
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
    type Error = TestError;

    async fn exec(&mut self, _oper: &FindUser<String, 1>) -> Result<String, Self::Error> {
        Ok(String::new())
    }
}

impl<'a, 'b> Proxy<UpdateUser<'a, 'b, String, 1>> for DummyProxy {
    type Error = TestError;

    async fn exec(&mut self, _oper: &UpdateUser<'a, 'b, String, 1>) -> Result<String, Self::Error> {
        Ok(String::new())
    }
}

fn assert_user_proxies<T, const N: usize>()
where
    T: Send + Sync,
    DummyProxy: UserRepoRunProxy<T, N> + UserRepoStepProxy<T, N>,
{
}

#[test]
fn generic_context_proxy_trait_drops_context_param() {
    assert_user_repo::<Context, String, 1>();
    assert_user_proxies::<String, 1>();
}
