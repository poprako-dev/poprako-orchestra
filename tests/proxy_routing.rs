#![cfg(feature = "macro")]
#![allow(refining_impl_trait)]

use std::future::{Ready, ready};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

use poprako_orchestra::{Context, Level, Oper, Proxy, Run, Step, proxy};

struct Serializable;
impl Level for Serializable {}

struct Cx;
impl Context for Cx {
    type Level = Serializable;
}

#[derive(Oper)]
#[oper(output = usize)]
struct Read<'a> {
    value: &'a str,
}

#[derive(Oper)]
#[oper(output = usize)]
struct Write<const N: usize>;

#[derive(Oper)]
#[oper(output = usize)]
struct FindUser<T, const N: usize> {
    marker: PhantomData<T>,
}

struct RunRepo<'a>(&'a AtomicUsize);
impl Run<Read<'_>> for RunRepo<'_> {
    type Error = ();

    fn run(&self, oper: &Read<'_>) -> Ready<Result<usize, Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(oper.value.len()))
    }
}

struct StepRepo<'a>(&'a AtomicUsize);
impl Step<Write<3>, Cx> for StepRepo<'_> {
    type Level = Serializable;
    type Error = ();

    fn step(&self, _context: &mut Cx, _oper: &Write<3>) -> Ready<Result<usize, Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(3))
    }
}

struct GenericRepo;
impl<T: Send + Sync, const N: usize> Run<FindUser<T, N>> for GenericRepo {
    type Error = ();

    async fn run(&self, _oper: &FindUser<T, N>) -> Result<usize, Self::Error> {
        Ok(N)
    }
}

#[test]
fn run_adapter_uses_only_run_provider_and_borrows_provider_once() {
    let calls = AtomicUsize::new(0);
    let repo = &RunRepo(&calls);
    let mut proxy = proxy! {
        run {
            repo => for<'a> Read<'a>;
        }
    };
    let result = futures::executor::block_on(proxy.exec(&Read { value: "abc" }));
    assert_eq!(result, Ok(3));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn step_adapter_uses_one_context_and_supports_const_binders() {
    let calls = AtomicUsize::new(0);
    let repo = &StepRepo(&calls);
    let context = &mut Cx;
    let mut proxy = proxy! {
        step(context) {
            repo => for<const N: usize> Write<N>;
        }
    };
    let result = futures::executor::block_on(proxy.exec(&Write::<3>));
    assert_eq!(result, Ok(3));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn run_and_step_are_independent_assemblies() {
    let run_calls = AtomicUsize::new(0);
    let step_calls = AtomicUsize::new(0);
    let run_repo = &RunRepo(&run_calls);
    let step_repo = &StepRepo(&step_calls);
    let mut run_adapter = proxy! {
        run {
            run_repo => for<'a> Read<'a>;
        }
    };
    let context = &mut Cx;
    let mut step_adapter = proxy! {
        step(context) {
            step_repo => for<const N: usize> Write<N>;
        }
    };
    assert_eq!(
        futures::executor::block_on(run_adapter.exec(&Read { value: "x" })),
        Ok(1)
    );
    assert_eq!(
        futures::executor::block_on(step_adapter.exec(&Write::<3>)),
        Ok(3)
    );
}

#[test]
fn proxy_supports_type_and_const_binders_together() {
    let repo = &GenericRepo;
    let mut proxy = proxy! {
        run {
            repo => for<T: Send, const N: usize> FindUser<T, N>;
        }
    };
    let oper = FindUser::<String, 7> {
        marker: PhantomData,
    };
    assert_eq!(futures::executor::block_on(proxy.exec(&oper)), Ok(7));
}
