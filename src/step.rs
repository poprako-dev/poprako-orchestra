//! Defines the [`Step`] and [`Run`] traits — the "how" of a transactional (or
//! non-transactional) operation.
//!
//! # [`Step`] — inside a transaction
//!
//! A [`Step`] is an *async executor* that processes a given [`Oper`] against a
//! mutable context. It encapsulates the logic of running one atomic unit of
//! work inside a transaction.
//!
//! # [`Run`] — self-contained execution
//!
//! A [`Run`] is an *async executor* that processes an [`Oper`] directly,
//! without a caller-provided context and therefore has no transaction level.

use std::future::Future;

use crate::level::{Context, Level, LevelGuard};
use crate::oper::Oper;

/// An async executor that processes an [`Oper`] against a given context.
///
/// * `O` — the [`Oper`] type this step can execute.
/// * `C` — the [`Context`] type this step requires.
pub trait Step<O, C>
where
    O: Oper,
    C: Context,
{
    /// The minimum transaction level required by this execution strategy.
    ///
    /// The level is local to **one** `Step` implementation: the same stepper
    /// may declare different levels on different operations.
    type Level: Level;

    /// Error type that may occur during step execution.
    type Error;

    /// Execute the operation against the given context and return its output.
    fn step(
        &self,
        cx: &mut C,
        oper: &O,
    ) -> impl Future<Output = Result<O::Output, Self::Error>> + Send
    where
        Self: LevelGuard<<C as Context>::Level, Self::Level>;
}

/// An [`Oper`] extension trait that invokes a [`Step`] from the operation.
///
/// Enable the `oper_ext` feature and import [`OperStep`] to write
/// `oper.step_on(repo, context)` instead of `repo.step(context, &oper)`.
/// The operation remains borrowed for the duration of the returned future.
#[cfg(feature = "oper_ext")]
pub trait OperStep<C>: Oper
where
    C: Context,
{
    /// Executes this operation with `repo` and `cx`.
    ///
    /// This is equivalent to `repo.step(cx, self)`.
    fn step_on<'a, R>(
        &'a self,
        repo: &'a R,
        cx: &'a mut C,
    ) -> impl Future<Output = Result<Self::Output, <R as Step<Self, C>>::Error>> + Send + 'a
    where
        Self: Sized,
        R: Step<Self, C> + ?Sized,
        R: LevelGuard<<C as Context>::Level, R::Level>,
    {
        repo.step(cx, self)
    }
}

#[cfg(feature = "oper_ext")]
impl<O, C> OperStep<C> for O
where
    O: Oper,
    C: Context,
{
}

/// A self-contained executor that processes an [`Oper`] directly.
///
/// Use `#[drive(...)]` (with this crate's `macro` feature enabled) to define
/// an empty aggregate trait over one or more [`Run`] and [`Step`] bounds.
pub trait Run<O>
where
    O: Oper,
{
    /// Error type that may occur during execution.
    type Error;

    /// Execute the operation and return its output.
    fn run(&self, oper: &O) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}

/// An [`Oper`] extension trait that invokes a [`Run`] from the operation.
///
/// Enable the `oper_ext` feature and import [`OperRun`] to write
/// `oper.run_on(repo)` instead of `repo.run(&oper)`. The operation remains
/// borrowed for the duration of the returned future.
#[cfg(feature = "oper_ext")]
pub trait OperRun: Oper {
    /// Executes this operation with `repo`.
    ///
    /// This is equivalent to `repo.run(self)`.
    fn run_on<'a, R>(
        &'a self,
        repo: &'a R,
    ) -> impl Future<Output = Result<Self::Output, <R as Run<Self>>::Error>> + Send + 'a
    where
        Self: Sized,
        R: Run<Self> + ?Sized,
    {
        repo.run(self)
    }
}

#[cfg(feature = "oper_ext")]
impl<O> OperRun for O where O: Oper {}
