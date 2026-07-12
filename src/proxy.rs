use crate::Oper;

/// Executes an [`Oper`] through a repository-backed proxy.
///
/// Proxy implementations normally forward to [`Run`](crate::Run) or
/// [`Step`](crate::Step), allowing business logic to depend only on the
/// operations it needs rather than a concrete repository type.
pub trait Proxy<O>
where
    O: Oper,
{
    /// Error returned when the proxied operation cannot be executed.
    type Error;

    /// Executes `oper` through the proxy's underlying repository.
    fn exec(&mut self, oper: &O) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}

/// Builds a proxy that dispatches operations through [`Run`](crate::Run).
///
/// Each row associates a repository identifier with the operation types it
/// executes. An operation with borrowed data may declare one or more
/// lifetimes with `for<'a, 'b>`, for example:
///
/// ```
/// # use poprako_orchestra::{Oper, Run, run_proxy};
/// # struct Repo;
/// # struct BorrowedOper<'a, 'b>(&'a str, &'b str);
/// # impl Oper for BorrowedOper<'_, '_> {
/// #     type Output = ();
/// # }
/// # impl Run<BorrowedOper<'_, '_>> for Repo {
/// #     type Error = ();
/// #
/// #     async fn run(&self, _oper: &BorrowedOper<'_, '_>) -> Result<(), Self::Error> {
/// #         Ok(())
/// #     }
/// # }
/// let repo = &Repo;
/// let _proxy = run_proxy! {
///     repo => for<'a, 'b> BorrowedOper<'a, 'b>;
/// };
/// ```
#[macro_export]
macro_rules! run_proxy {
    (
        $(
            $repo:ident => $(for<$($oper_lt:lifetime),+> $oper:ty),+ $(,)?;
        )+
    ) => {{
        #[allow(non_camel_case_types)]
        struct RunProxy<'run_proxy_repo, $($repo),+> {
            $(
                $repo: &'run_proxy_repo $repo,
            )+
        }

        $crate::run_proxy! {
            @impl_lifetime_rows
            RunProxy,
            [$($repo),+];

            $(
                $repo => $(for<$($oper_lt),+> $oper),+;
            )+
        }

        RunProxy {
            $($repo),+
        }
    }};

    (
        $(
            $repo:ident => $oper:ty $(, $oper_rest:ty)* $(,)?;
        )+
    ) => {{
        #[allow(non_camel_case_types)]
        struct RunProxy<'run_proxy_repo, $($repo),+> {
            $(
                $repo: &'run_proxy_repo $repo,
            )+
        }

        $crate::run_proxy! {
            @impl_plain_rows
            RunProxy,
            [$($repo),+];

            $(
                $repo => $oper $(, $oper_rest)*;
            )+
        }

        RunProxy {
            $($repo),+
        }
    }};

    (
        @impl_lifetime_rows
        $proxy:ident,
        $all_repos:tt;
    ) => {};

    (
        @impl_lifetime_rows
        $proxy:ident,
        $all_repos:tt;

        $repo:ident => $(for<$($oper_lt:lifetime),+> $oper:ty),+;

        $($rest:tt)*
    ) => {
        $(
            $crate::run_proxy! {
                @impl_lifetime_one
                $proxy,
                $all_repos,
                $repo;

                for<$($oper_lt),+> $oper
            }
        )+

        $crate::run_proxy! {
            @impl_lifetime_rows
            $proxy,
            $all_repos;

            $($rest)*
        }
    };

    (
        @impl_lifetime_one
        $proxy:ident,
        [$($all_repo:ident),+],
        $repo:ident;

        for<$($oper_lt:lifetime),+> $oper:ty
    ) => {
        #[allow(non_camel_case_types)]
        impl<$($oper_lt,)+ 'run_proxy_repo, $($all_repo),+> $crate::Proxy<$oper>
            for $proxy<'run_proxy_repo, $($all_repo),+>
        where
            $repo: $crate::Run<$oper>,
        {
            type Error = <$repo as $crate::Run<$oper>>::Error;

            fn exec(
                &mut self,
                oper: &$oper,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<
                    <$oper as $crate::Oper>::Output,
                    Self::Error,
                >,
            > + Send {
                <$repo as $crate::Run<$oper>>::run(self.$repo, oper)
            }
        }
    };

    (
        @impl_plain_rows
        $proxy:ident,
        $all_repos:tt;
    ) => {};

    (
        @impl_plain_rows
        $proxy:ident,
        $all_repos:tt;

        $repo:ident => $($oper:ty),+;

        $($rest:tt)*
    ) => {
        $(
            $crate::run_proxy! {
                @impl_plain_one
                $proxy,
                $all_repos,
                $repo;

                $oper
            }
        )+

        $crate::run_proxy! {
            @impl_plain_rows
            $proxy,
            $all_repos;

            $($rest)*
        }
    };

    (
        @impl_plain_one
        $proxy:ident,
        [$($all_repo:ident),+],
        $repo:ident;

        $oper:ty
    ) => {
        #[allow(non_camel_case_types)]
        impl<'run_proxy_repo, $($all_repo),+> $crate::Proxy<$oper>
            for $proxy<'run_proxy_repo, $($all_repo),+>
        where
            $repo: $crate::Run<$oper>,
        {
            type Error = <$repo as $crate::Run<$oper>>::Error;

            fn exec(
                &mut self,
                oper: &$oper,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<
                    <$oper as $crate::Oper>::Output,
                    Self::Error,
                >,
            > + Send {
                <$repo as $crate::Run<$oper>>::run(self.$repo, oper)
            }
        }
    };
}

/// Builds a proxy that dispatches operations through [`Step`](crate::Step).
///
/// The first argument is the transaction context. Each following row maps a
/// repository identifier to the operation types it can step. Borrowed
/// operations may declare any positive number of lifetimes with
/// `for<'a, 'b, ...>`.
#[macro_export]
macro_rules! step_proxy {
    (
        $context:expr;

        $(
            $repo:ident => $(for<$($oper_lt:lifetime),+> $oper:ty),+ $(,)?;
        )+
    ) => {{
        #[allow(non_camel_case_types)]
        struct StepProxy<
            'step_proxy_context,
            'step_proxy_repo,
            StepProxyContext,
            $($repo),+
        > {
            // The proxy owns the only long-lived mutable context reference.
            // Each `Proxy::exec` implementation reborrows it for one step.
            context: &'step_proxy_context mut StepProxyContext,

            $(
                $repo: &'step_proxy_repo $repo,
            )+
        }

        $crate::step_proxy! {
            @impl_rows
            StepProxy,
            StepProxyContext,
            [$($repo),+];

            $(
                $repo => $(for<$($oper_lt),+> $oper),+;
            )+
        }

        StepProxy {
            context: &mut *$context,
            $($repo),+
        }
    }};

    (
        $context:expr;

        $(
            $repo:ident => $oper:ty $(, $oper_rest:ty)* $(,)?;
        )+
    ) => {{
        #[allow(non_camel_case_types)]
        struct StepProxy<
            'step_proxy_context,
            'step_proxy_repo,
            StepProxyContext,
            $($repo),+
        > {
            // The proxy owns the only long-lived mutable context reference.
            // Each `Proxy::exec` implementation reborrows it for one step.
            context: &'step_proxy_context mut StepProxyContext,

            $(
                $repo: &'step_proxy_repo $repo,
            )+
        }

        $crate::step_proxy! {
            @impl_plain_rows
            StepProxy,
            StepProxyContext,
            [$($repo),+];

            $(
                $repo => $oper $(, $oper_rest)*;
            )+
        }

        StepProxy {
            context: &mut *$context,
            $($repo),+
        }
    }};

    // Finishes processing all repository rows.
    (
        @impl_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;
    ) => {};

    // Generates implementations for one lifetime-bearing repository row and continues.
    (
        @impl_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;

        $repo:ident => $(for<$($oper_lt:lifetime),+> $oper:ty),+;

        $($rest:tt)*
    ) => {
        $crate::step_proxy! {
            @impl_operations
            $proxy,
            $context_ty,
            $all_repos,
            $repo;

            $(for<$($oper_lt),+> $oper),+
        }

        $crate::step_proxy! {
            @impl_rows
            $proxy,
            $context_ty,
            $all_repos;

            $($rest)*
        }
    };

    // Generates one `Proxy` implementation per lifetime-bearing operation.
    (
        @impl_operations
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt,
        $repo:ident;

        $(for<$($oper_lt:lifetime),+> $oper:ty),+
    ) => {
        $(
            $crate::step_proxy! {
                @impl_one
                $proxy,
                $context_ty,
                $all_repos,
                $repo;

                for<$($oper_lt),+> $oper
            }
        )+
    };

    // Finishes processing all plain-operation repository rows.
    (
        @impl_plain_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;
    ) => {};

    // Generates implementations for one plain-operation repository row and continues.
    (
        @impl_plain_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;

        $repo:ident => $($oper:ty),+;

        $($rest:tt)*
    ) => {
        $(
            $crate::step_proxy! {
                @impl_plain_one
                $proxy,
                $context_ty,
                $all_repos,
                $repo;

                $oper
            }
        )+

        $crate::step_proxy! {
            @impl_plain_rows
            $proxy,
            $context_ty,
            $all_repos;

            $($rest)*
        }
    };

    // Generates a `Proxy` implementation for a borrowed operation.
    (
        @impl_one
        $proxy:ident,
        $context_ty:ident,
        [$($all_repo:ident),+],
        $repo:ident;

        for<$($oper_lt:lifetime),+> $oper:ty
    ) => {
        #[allow(non_camel_case_types)]
        impl<
            $($oper_lt,)+
            'step_proxy_context,
            'step_proxy_repo,
            $context_ty,
            $($all_repo),+
        > $crate::Proxy<$oper>
            for $proxy<
                'step_proxy_context,
                'step_proxy_repo,
                $context_ty,
                $($all_repo),+
            >
        where
            $repo: $crate::Step<$oper, $context_ty>,
        {
            type Error =
                <$repo as $crate::Step<$oper, $context_ty>>::Error;

            fn exec(
                &mut self,
                oper: &$oper,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<
                    <$oper as $crate::Oper>::Output,
                    Self::Error,
                >,
            > + Send {
                <$repo as $crate::Step<$oper, $context_ty>>::step(
                    self.$repo,
                    &mut *self.context,
                    oper,
                )
            }
        }
    };

    // Generates a `Proxy` implementation for a plain operation.
    (
        @impl_plain_one
        $proxy:ident,
        $context_ty:ident,
        [$($all_repo:ident),+],
        $repo:ident;

        $oper:ty
    ) => {
        #[allow(non_camel_case_types)]
        impl<
            'step_proxy_context,
            'step_proxy_repo,
            $context_ty,
            $($all_repo),+
        > $crate::Proxy<$oper>
            for $proxy<
                'step_proxy_context,
                'step_proxy_repo,
                $context_ty,
                $($all_repo),+
            >
        where
            $repo: $crate::Step<$oper, $context_ty>,
        {
            type Error =
                <$repo as $crate::Step<$oper, $context_ty>>::Error;

            fn exec(
                &mut self,
                oper: &$oper,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<
                    <$oper as $crate::Oper>::Output,
                    Self::Error,
                >,
            > + Send {
                <$repo as $crate::Step<$oper, $context_ty>>::step(
                    self.$repo,
                    &mut *self.context,
                    oper,
                )
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Run, Step};

    struct PlainOper;

    impl Oper for PlainOper {
        type Output = ();
    }

    struct BorrowedOper<'a>(&'a str);

    impl Oper for BorrowedOper<'_> {
        type Output = ();
    }

    struct DoublyBorrowedOper<'first, 'second>(&'first str, &'second str);

    impl Oper for DoublyBorrowedOper<'_, '_> {
        type Output = ();
    }

    struct Repo;

    impl Run<PlainOper> for Repo {
        type Error = ();

        async fn run(&self, _oper: &PlainOper) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Run<BorrowedOper<'_>> for Repo {
        type Error = ();

        async fn run(&self, oper: &BorrowedOper<'_>) -> Result<(), Self::Error> {
            let _ = oper.0;
            Ok(())
        }
    }

    impl Run<DoublyBorrowedOper<'_, '_>> for Repo {
        type Error = ();

        async fn run(&self, oper: &DoublyBorrowedOper<'_, '_>) -> Result<(), Self::Error> {
            let _ = oper.0;
            let _ = oper.1;

            Ok(())
        }
    }

    impl Step<DoublyBorrowedOper<'_, '_>, ()> for Repo {
        type Error = ();

        async fn step(
            &self,
            _context: &mut (),
            oper: &DoublyBorrowedOper<'_, '_>,
        ) -> Result<(), Self::Error> {
            let _ = oper.0;
            let _ = oper.1;

            Ok(())
        }
    }

    #[test]
    fn run_proxy_supports_plain_oper() {
        let repo = &Repo;
        let mut proxy = run_proxy! {
            repo => PlainOper;
        };

        drop(proxy.exec(&PlainOper));
    }

    #[test]
    fn run_proxy_supports_borrowed_oper() {
        let repo = &Repo;
        let mut proxy = run_proxy! {
            repo => for<'a> BorrowedOper<'a>;
        };

        drop(proxy.exec(&BorrowedOper("value")));
    }

    #[test]
    fn run_proxy_supports_multiple_borrowed_lifetimes() {
        let repo = &Repo;
        let mut proxy = run_proxy! {
            repo => for<'first, 'second> DoublyBorrowedOper<'first, 'second>;
        };

        drop(proxy.exec(&DoublyBorrowedOper("first", "second")));
    }

    #[test]
    fn step_proxy_supports_multiple_borrowed_lifetimes() {
        let repo = &Repo;
        let mut context = ();
        let context = &mut context;
        let mut proxy = step_proxy! {
            context;
            repo => for<'first, 'second> DoublyBorrowedOper<'first, 'second>;
        };

        drop(proxy.exec(&DoublyBorrowedOper("first", "second")));
    }
}
