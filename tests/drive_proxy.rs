#![cfg(feature = "macro")]
#![allow(dead_code)]

use std::marker::PhantomData;

use poprako_orchestra::{AtLeast, Context, Level, Oper, Proxy, Run, Step, drive, proxy};

struct Transactional;

impl Level for Transactional {}

struct Cx;

impl Context for Cx {
    type Level = Transactional;
}

// --- Concrete-context case: one proxy capability, mixed execution wiring ---

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

// The capability is the union, while each operation keeps its execution mode.
#[drive(
    context = Cx,
    error = String,
    proxy = OrderRepoProxy,
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
fn one_proxy_capability_supports_asymmetric_execution_wiring() {
    require_repo::<Repo>();

    let repo = &Repo;
    let mut context = Cx;
    let mut proxy = proxy! {
        run => repo as OrderRepoProxy;
        step(&mut context) => repo as OrderRepoProxy;
    };

    require_proxy(&proxy);
    place(&mut proxy);
    drop(proxy.exec(&EnsureCustomer { customer_id: "c" }));
    drop(proxy.exec(&CreateOrder {
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

#[derive(Oper)]
#[oper(output = ())]
struct BorrowedGeneric<'a, T, const N: usize> {
    value: &'a T,
}

#[drive(
    error = TestError,
    proxy = GenericRepoProxy,
    run(for<'a> BorrowedGeneric<'a, T, N>),
)]
trait GenericRepoDrive<T, const N: usize> {}

impl<T, const N: usize> Run<BorrowedGeneric<'_, T, N>> for GenericRepo
where
    T: Sync,
{
    type Error = TestError;

    async fn run(&self, oper: &BorrowedGeneric<'_, T, N>) -> Result<(), Self::Error> {
        let _ = oper.value;
        Ok(())
    }
}

fn accept_generic_proxy<T, const N: usize>(repo: &GenericRepo, value: &T)
where
    T: Sync,
{
    let mut proxy = proxy! {
        run => repo as GenericRepoProxy;
    };
    drop(proxy.exec(&BorrowedGeneric::<T, N> { value }));
}

#[test]
fn capability_table_preserves_borrowed_generic_and_const_opers() {
    accept_generic_proxy::<String, 7>(&GenericRepo, &String::new());
}
