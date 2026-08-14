#![cfg(feature = "macro")]

use std::marker::PhantomData;

use poprako_orchestra::{
    AtLeast, Context, Level, Oper, Proxy, Run, Step, drive, run_proxy, step_proxy,
};

struct Transactional;

impl Level for Transactional {}

struct Cx;

impl Context for Cx {
    type Level = Transactional;
}

// --- Concrete-context case: one proxy capability, two transaction wirings ---

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

// One capability trait merges both operation lists. Complex logic depends on
// this single name and never learns whether an operation is run or stepped.
#[drive(
    context = Cx,
    error = String,
    proxy = OrderRepoProxy,
    run(for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>),
    step(for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>),
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

impl Run<CreateOrder<'_>> for Repo {
    type Error = String;

    async fn run(&self, oper: &CreateOrder<'_>) -> Result<u64, Self::Error> {
        Ok(oper.customer_id.len() as u64 + oper.quantity as u64)
    }
}

impl Step<EnsureCustomer<'_>, Cx> for Repo {
    type Level = Transactional;
    type Error = String;

    async fn step(&self, _context: &mut Cx, _oper: &EnsureCustomer<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Step<CreateOrder<'_>, Cx> for Repo {
    type Level = Transactional;
    type Error = String;

    async fn step(&self, _context: &mut Cx, oper: &CreateOrder<'_>) -> Result<u64, Self::Error> {
        Ok(oper.customer_id.len() as u64 + oper.quantity as u64)
    }
}

fn require_repo<R: OrderRepo>() {}

fn require_proxy<P: OrderRepoProxy>(_proxy: &P) {}

// Complex: one capability name — no Run/Step/Context/Level anywhere.
fn place<P: OrderRepoProxy>(_proxy: &mut P) {}

#[test]
fn one_proxy_capability_satisfied_by_both_transaction_wirings() {
    require_repo::<Repo>();

    let repo = &Repo;

    // Run wiring: every operation executes non-transactionally.
    let mut run_proxy = run_proxy! {
        repo => for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>;
    };
    require_proxy(&run_proxy);
    place(&mut run_proxy);
    drop(run_proxy.exec(&EnsureCustomer { customer_id: "c" }));
    drop(run_proxy.exec(&CreateOrder {
        customer_id: "c",
        quantity: 1,
    }));

    // Step wiring: the same operations execute inside one shared transaction
    // context. The capability trait cannot tell the two wirings apart.
    let mut context = Cx;
    let context = &mut context;
    let mut step_proxy = step_proxy! {
        context;
        repo => for<'a> EnsureCustomer<'a>, for<'a> CreateOrder<'a>;
    };
    require_proxy(&step_proxy);
    place(&mut step_proxy);
    drop(step_proxy.exec(&EnsureCustomer { customer_id: "c" }));
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

// The proxy trait drops `C` and merges the run and step operation sets.
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
    type Error = TestError;

    async fn run(&self, _oper: &FindUser<T, N>) -> Result<T, Self::Error> {
        panic!()
    }
}

impl<'a, 'b, C, T, const N: usize> Step<UpdateUser<'a, 'b, T, N>, C> for GenericRepo
where
    C: Context + Send,
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
    C: Context + Send,
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

fn assert_user_proxy<T, const N: usize>()
where
    T: Send + Sync,
    DummyProxy: UserRepoProxy<T, N>,
{
}

#[test]
fn generic_context_proxy_trait_drops_context_param() {
    assert_user_repo::<Cx, String, 1>();
    assert_user_proxy::<String, 1>();
}
