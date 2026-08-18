# poprako-orchestra

`poprako-orchestra` provides small, independent Rust traits for operations,
non-transactional execution, transactional steps, and transaction contexts.

## Execution traits

`Run<O>` and `Step<O, C>` are deliberately independent. A run provider does
not need a context or a `Step` implementation, and a step provider does not
need a `Run` implementation.

```rust
pub trait Run<O: Oper> {
    type Error;
    fn run(&self, oper: &O) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}

pub trait Step<O: Oper, C: Context> {
    type Level: Level;
    type Error;
    fn step(&self, context: &mut C, oper: &O)
        -> impl Future<Output = Result<O::Output, Self::Error>> + Send
    where
        Self: LevelGuard<C::Level, Self::Level>;
}
```

## Explicit operation proxies

With the `macro` feature, `proxy!` creates one stack-local proxy. Every
operation is listed explicitly, so a generic complex can state exactly what
it needs:

```rust
async fn complex<P>(proxy: &mut P)
where
    P: for<'a> Proxy<FindMemberInfo<'a>, Error = BaseError>
        + for<'a> Proxy<UpdateMember<'a>, Error = BaseError>,
{
    // operation-specific calls through `proxy`
}
```

Run-only wiring:

```rust
let mut proxy = proxy! {
    run {
        repo => for<'a> FindMemberInfo<'a>;
    }
};
```

Step wiring can use multiple providers and borrows the context once:

```rust
let context = &mut context;
let mut proxy = proxy! {
    step(context) {
        repo => for<'a> FindMemberInfo<'a>, for<'a> UpdateMember<'a>;
        prom => for<'a> RecordPromotion<'a>;
    }
};
```

Type, lifetime, and const parameters use one binder:

```rust
let mut proxy = proxy! {
    run {
        repo => for<T: Send, const N: usize> FindUser<T, N>,
                for<'a, T: Send, const N: usize> UpdateUser<'a, T, N>;
    }
};
```

The two proxy modes are separate assemblies. There is no priority routing,
capability-union trait, hidden collector macro, implicit commit type, or
`Run -> Step` bridge. `#[drive]` only generates repository aggregate traits
for its declared `run(...)` and `step(...)` operations; it does not accept a
`proxy = ...` argument.

## Features

- `macro`: enables `#[derive(Oper)]`, `#[drive]`, and `proxy!`.
- `oper_ext`: enables `run_on`, `step_on`, and `proxy_on` helpers.

See [`examples/proxy-complicated`](examples/proxy-complicated) for a complete
run-only and step-only wiring example.
