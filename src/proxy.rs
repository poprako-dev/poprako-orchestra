use crate::Oper;

pub trait Proxy<O>
where
    O: Oper,
{
    type Error;

    fn exec(&mut self, oper: &O) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}

#[macro_export]
macro_rules! run_proxy {
    (
        $(
            $repo:ident => $(for<$oper_lt:lifetime> $oper:ty),+ $(,)?;
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
                $repo => $(for<$oper_lt> $oper),+;
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

        $repo:ident => $(for<$oper_lt:lifetime> $oper:ty),+;

        $($rest:tt)*
    ) => {
        $(
            $crate::run_proxy! {
                @impl_lifetime_one
                $proxy,
                $all_repos,
                $repo;

                for<$oper_lt> $oper
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

        for<$oper_lt:lifetime> $oper:ty
    ) => {
        #[allow(non_camel_case_types)]
        impl<$oper_lt, 'run_proxy_repo, $($all_repo),+> $crate::Proxy<$oper>
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

#[macro_export]
macro_rules! step_proxy {
    (
        $context:expr;

        $(
            $repo:ident => $(for<$oper_lt:lifetime> $oper:ty),+ $(,)?;
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
            // Each `Proxy::exec` implementation reborrows it for one Step call.
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
                $repo => $(for<$oper_lt> $oper),+;
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
            $repo:ident => $(
                $(for<$oper_lt:lifetime>)? $oper:ty
            ),+ $(,)?;
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
            // Each `Proxy::exec` implementation reborrows it for one Step call.
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
                $repo => $(
                    $(for<$oper_lt>)? $oper
                ),+;
            )+
        }

        StepProxy {
            context: &mut *$context,
            $($repo),+
        }
    }};

    // 所有 repo 行处理完毕。
    (
        @impl_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;
    ) => {};

    // 取出只包含带生命周期 operation 的 repo 行。
    (
        @impl_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;

        $repo:ident => $(for<$oper_lt:lifetime> $oper:ty),+;

        $($rest:tt)*
    ) => {
        $crate::step_proxy! {
            @impl_operations
            $proxy,
            $context_ty,
            $all_repos,
            $repo;

            $(for<$oper_lt> $oper),+
        }

        $crate::step_proxy! {
            @impl_rows
            $proxy,
            $context_ty,
            $all_repos;

            $($rest)*
        }
    };

    // 取出一行 repo => operations，然后继续处理剩余行。
    (
        @impl_rows
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt;

        $repo:ident => $(
            $(for<$oper_lt:lifetime>)? $oper:ty
        ),+;

        $($rest:tt)*
    ) => {
        $crate::step_proxy! {
            @impl_operations
            $proxy,
            $context_ty,
            $all_repos,
            $repo;

            $(
                $(for<$oper_lt>)? $oper
            ),+
        }

        $crate::step_proxy! {
            @impl_rows
            $proxy,
            $context_ty,
            $all_repos;

            $($rest)*
        }
    };

    // 为只包含带生命周期 operation 的 repo 行生成 Proxy impl。
    (
        @impl_operations
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt,
        $repo:ident;

        $(for<$oper_lt:lifetime> $oper:ty),+
    ) => {
        $(
            $crate::step_proxy! {
                @impl_one
                $proxy,
                $context_ty,
                $all_repos,
                $repo;

                for<$oper_lt> $oper
            }
        )+
    };

    // 为当前 repo 的每个 operation 分别生成一个 Proxy impl。
    (
        @impl_operations
        $proxy:ident,
        $context_ty:ident,
        $all_repos:tt,
        $repo:ident;

        $(
            $(for<$oper_lt:lifetime>)? $oper:ty
        ),+
    ) => {
        $(
            $crate::step_proxy! {
                @impl_one
                $proxy,
                $context_ty,
                $all_repos,
                $repo;

                $(for<$oper_lt>)? $oper
            }
        )+
    };

    // 带生命周期参数的 Oper。
    (
        @impl_one
        $proxy:ident,
        $context_ty:ident,
        [$($all_repo:ident),+],
        $repo:ident;

        for<$oper_lt:lifetime> $oper:ty
    ) => {
        #[allow(non_camel_case_types)]
        impl<
            $oper_lt,
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

    // 不带生命周期参数的 Oper。
    (
        @impl_one
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

    use crate::Run;

    struct PlainOper;

    impl Oper for PlainOper {
        type Output = ();
    }

    struct BorrowedOper<'a>(&'a str);

    impl Oper for BorrowedOper<'_> {
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
}
