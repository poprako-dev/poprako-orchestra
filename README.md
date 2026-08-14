# poprako-orchestra

A unified transaction abstraction framework for Rust with compile-time transaction-level checks.

## Overview

`poprako-orchestra` decouples transactional logic into three independent concerns:

| Trait  | Role                        | Module              |
|--------|-----------------------------| ------------------- |
| `Oper` | **What** — the operation's input and output | [`oper`] |
| `Step` | **How** — transactional executor declaring its required level | [`step`] |
| `Nucl` | **Where** — backend providing a scoped context and actual level | [`nucl`] |

Plus a non-transactional variant:

| Trait  | Role                        | Module              |
|--------|-----------------------------| ------------------- |
| `Run`  | **How (self-contained)** — non-transactional executor | [`step`] |

Transaction levels are application-defined marker types. `AtLeast<Required>`
expresses compatibility, while `Context` associates an execution context with
the level it actually provides. The framework supplies only the reflexive
relationship, so stronger levels explicitly declare every weaker level they
satisfy.

## Quick example

```rust
use poprako_orchestra::{AtLeast, Level, Oper, Context, Step};
use poprako_orchestra::nucl::{Nucl, NuclError};

pub struct RepeatableRead;
impl Level for RepeatableRead {}

pub struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

pub struct DbConn;
impl Context for DbConn {
    type Level = Serializable;
}

// 1. Define the operation (data)
pub struct CreateUser {
    pub name: String,
}

impl Oper for CreateUser {
    type Output = u64; // user id
}

// 2. Implement how to execute it
struct UserRepo;

impl Step<CreateUser, DbConn> for UserRepo {
    type Level = RepeatableRead;
    type Error = db::Error;

    async fn step(&self, cx: &mut DbConn, oper: &CreateUser) -> Result<u64, Self::Error> {
        sqlx::query("INSERT INTO users (name) VALUES ($1) RETURNING id")
            .bind(&oper.name)
            .fetch_one(cx)
            .await
            .map(|r| r.get("id"))
    }
}

// 3. Wire it through a transactional nucleus
async fn create_user(
    nucl: &impl Nucl<Level = Serializable, Context = DbConn>,
    repo: &UserRepo,
    name: String,
) {
    let result = nucl.coord(async |cx| {
        repo.step(cx, &CreateUser { name }).await
    }).await; // Result<u64, NuclError<db::Error, db::Error>>
}
```

With the `macro` feature, operations declare only their output:
`#[oper(output = u64)]`.

`#[drive]` generates **one** proxy capability trait that merges the
transaction-free and transactional operation lists. The proxy carries no
context and no level — it erases the transaction model:

```rust
#[drive(
    context = DbConn,
    error = db::Error,
    proxy = UserRepoProxy,
    run(GetUser),
    step(CreateUser),
)]
trait UserRepo {}
```

Complex logic depends on `P: UserRepoProxy` alone and never learns whether an
operation is run or stepped; the `run_proxy!` / `step_proxy!` combinators pick
the wiring at the application layer.

## Transaction levels

Each `Step` implementation declares its own required level, and that level is
local to **one** implementation — the same stepper may require
`RepeatableRead` for one operation and `Serializable` for another:

```rust
impl Step<CreateUser, DbConn> for UserRepo {
    type Level = RepeatableRead; // this operation only needs repeatable read
    // ...
}

impl Step<DeleteUser, DbConn> for UserRepo {
    type Level = Serializable; // this operation needs serializable
    // ...
}
```

`#[drive]` propagates every step requirement onto the aggregate trait as
`LevelGuard` supertraits, so a usecase needs only the capability bound plus a
single business-level declaration — never per-operation `AtLeast` bounds:

```rust
async fn purge_usecase<C, R>(cx: &mut C, repo: &R) -> Result<(), db::Error>
where
    C: Context<Level = Serializable>, // one declaration switches the flow level
    R: UserRepo<C>,                   // step requirements are implied by drive
{
    repo.step(cx, &DeleteUser { id: 1 }).await
}
```

Mismatches fail at compile time: a nucleus whose `C::Level` is weaker than a
step's requirement, or a usecase pinning a level below some step, both refuse
to compile. The proxy trait carries no level constraints, so complex logic
stays level-free.

Mechanically, `Step::step` is guarded by
`Self: LevelGuard<<C as Context>::Level, Self::Level>`, where `LevelGuard` is
a blanket-implemented marker: it holds for any stepper whose context satisfies
`C::Level: AtLeast<Step::Level>`. Because the guard is a bound on the stepper
itself, `#[drive]` can hoist it into the aggregate trait's supertraits — the
one position rustc assumes for callers — while the underlying `AtLeast`
relation is still enforced at concrete instantiation.

## Why separate Oper from Step?

- The same `Oper` can be executed by different `Step` implementations in different contexts.
- `Step` is a stateless executor — all per-call state lives in the `Oper` value.
- This is the **Command pattern**: `Oper` is the command object, `Step` is the handler.

## Version policy

- Rust edition **2024** — requires Rust 1.85+.
- Pre-1.0: minor versions may include breaking changes. Pin your version.

## License

Licensed under the [MIT License](LICENSE).
