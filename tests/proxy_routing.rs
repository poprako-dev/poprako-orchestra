#![cfg(feature = "macro")]
#![allow(dead_code, refining_impl_trait)]

use std::future::{Ready, ready};
use std::sync::atomic::{AtomicUsize, Ordering};

use poprako_orchestra::{Context, Level, Oper, Proxy, Run, Step, drive, proxy};

struct Transactional;

impl Level for Transactional {}

struct Cx;

impl Context for Cx {
    type Level = Transactional;
}

#[derive(Oper)]
#[oper(output = ())]
struct Read;

#[derive(Oper)]
#[oper(output = ())]
struct Write;

#[derive(Oper)]
#[oper(output = ())]
struct Shared;

#[drive(
    context = Cx,
    error = (),
    proxy = HybridProxy,
    run(Read, Shared),
    step(Write, Shared),
)]
trait Hybrid {}

struct RunRead<'a>(&'a AtomicUsize);

impl Run<Read> for RunRead<'_> {
    type Error = ();

    fn run(&self, _oper: &Read) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

struct StepWriteShared<'a>(&'a AtomicUsize);

impl Step<Write, Cx> for StepWriteShared<'_> {
    type Level = Transactional;
    type Error = ();

    fn step(&self, _context: &mut Cx, _oper: &Write) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

impl Step<Shared, Cx> for StepWriteShared<'_> {
    type Level = Transactional;
    type Error = ();

    fn step(&self, _context: &mut Cx, _oper: &Shared) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

#[test]
fn default_priority_selects_step_without_requiring_unselected_run() {
    let run_calls = AtomicUsize::new(0);
    let step_calls = AtomicUsize::new(0);
    let run_repo = &RunRead(&run_calls);
    let step_repo = &StepWriteShared(&step_calls);
    let mut context = Cx;
    let mut proxy = proxy! {
        run => run_repo as HybridProxy;
        step(&mut context) => step_repo as HybridProxy;
    };

    drop(proxy.exec(&Read));
    drop(proxy.exec(&Write));
    drop(proxy.exec(&Shared));

    assert_eq!(run_calls.load(Ordering::Relaxed), 1);
    assert_eq!(step_calls.load(Ordering::Relaxed), 2);
}

struct RunReadShared<'a>(&'a AtomicUsize);

impl Run<Read> for RunReadShared<'_> {
    type Error = ();

    fn run(&self, _oper: &Read) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

impl Run<Shared> for RunReadShared<'_> {
    type Error = ();

    fn run(&self, _oper: &Shared) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

struct StepWrite<'a>(&'a AtomicUsize);

impl Step<Write, Cx> for StepWrite<'_> {
    type Level = Transactional;
    type Error = ();

    fn step(&self, _context: &mut Cx, _oper: &Write) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

#[test]
fn explicit_priority_selects_run_without_requiring_unselected_step() {
    let run_calls = AtomicUsize::new(0);
    let step_calls = AtomicUsize::new(0);
    let run_repo = &RunReadShared(&run_calls);
    let step_repo = &StepWrite(&step_calls);
    let mut context = Cx;
    let mut proxy = proxy! {
        priority => run, step;
        run => run_repo as HybridProxy;
        step(&mut context) => step_repo as HybridProxy;
    };

    drop(proxy.exec(&Read));
    drop(proxy.exec(&Write));
    drop(proxy.exec(&Shared));

    assert_eq!(run_calls.load(Ordering::Relaxed), 2);
    assert_eq!(step_calls.load(Ordering::Relaxed), 1);
}

#[drive(error = (), proxy = ReadProxy, run(Read, Shared))]
trait ReadDrive {}

#[drive(
    context = Cx,
    error = (),
    proxy = WriteProxy,
    step(Write, Shared),
)]
trait WriteDrive {}

struct MultiRepo<'a>(&'a AtomicUsize);

impl Run<Read> for MultiRepo<'_> {
    type Error = ();

    fn run(&self, _oper: &Read) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

impl Run<Shared> for MultiRepo<'_> {
    type Error = ();

    fn run(&self, _oper: &Shared) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

impl Step<Write, Cx> for MultiRepo<'_> {
    type Level = Transactional;
    type Error = ();

    fn step(&self, _context: &mut Cx, _oper: &Write) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

impl Step<Shared, Cx> for MultiRepo<'_> {
    type Level = Transactional;
    type Error = ();

    fn step(&self, _context: &mut Cx, _oper: &Shared) -> Ready<Result<(), Self::Error>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ready(Ok(()))
    }
}

fn require_standard_capabilities<P>(_proxy: &P)
where
    P: ReadProxy + WriteProxy,
{
}

#[test]
fn one_provider_supplies_multiple_capabilities_and_duplicate_opers_are_deduped() {
    let calls = AtomicUsize::new(0);
    let repo = &MultiRepo(&calls);
    let mut context = Cx;
    let mut proxy = proxy! {
        run => repo as ReadProxy + WriteProxy;
        step(&mut context) => repo as ReadProxy + WriteProxy;
    };

    require_standard_capabilities(&proxy);
    drop(proxy.exec(&Read));
    drop(proxy.exec(&Write));
    drop(proxy.exec(&Shared));
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[test]
fn run_only_and_step_only_capability_proxies_compile() {
    let calls = AtomicUsize::new(0);
    let run_repo = &RunReadShared(&calls);
    let run_proxy = proxy! {
        run => run_repo as ReadProxy;
    };
    require_read_proxy(&run_proxy);

    let step_repo = &StepWriteShared(&calls);
    let mut context = Cx;
    let step_proxy = proxy! {
        step(&mut context) => step_repo as WriteProxy;
    };
    require_write_proxy(&step_proxy);
}

fn require_read_proxy<P: ReadProxy>(_proxy: &P) {}

fn require_write_proxy<P: WriteProxy>(_proxy: &P) {}
