# poprako-orchestra

A unified transaction abstraction framework for Rust with compile-time transaction-level checks.

## Overview

`poprako-orchestra` decouples transactional logic into three independent concerns:

| Trait  | Role                        | Module              |
|--------|-----------------------------| ------------------- |
| `Oper` | **What** — the operation's input, output, and minimum level | [`oper`] |
| `Step` | **How** — async executor running inside a transaction | [`step`] |
| `Nucl` | **Where** — backend providing a scoped context and actual level | [`nucl`] |

Plus a non-transactional variant:

| Trait  | Role                        | Module              |
|--------|-----------------------------| ------------------- |
| `Run`  | **How (self-contained)** — executor declaring its actual guarantee | [`step`] |

Transaction levels are application-defined marker types. `AtLeast<Required>`
expresses compatibility, while `Scope` associates a context with the level it
actually provides. The framework supplies only the reflexive relationship, so
stronger levels explicitly declare every weaker level they satisfy.

## Quick example

```rust
use poprako_orchestra::{AtLeast, Level, Oper, Scope, Step};
use poprako_orchestra::nucl::{Nucl, NuclError};

pub struct RepeatableRead;
impl Level for RepeatableRead {}

pub struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

pub struct DbConn;
impl Scope for DbConn {
    type Level = Serializable;
}

// 1. Define the operation (data)
pub struct CreateUser {
    pub name: String,
}

impl Oper for CreateUser {
    type Output = u64; // user id
    type Level = RepeatableRead;
}

// 2. Implement how to execute it
struct UserRepo;

impl Step<CreateUser, DbConn> for UserRepo {
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

With the `macro` feature, operations use both required fields (in either
order): `#[oper(output = u64, level = RepeatableRead)]`.

## Why separate Oper from Step?

- The same `Oper` can be executed by different `Step` implementations in different contexts.
- `Step` is a stateless executor — all per-call state lives in the `Oper` value.
- This is the **Command pattern**: `Oper` is the command object, `Step` is the handler.

## Version policy

- Rust edition **2024** — requires Rust 1.85+.
- Pre-1.0: minor versions may include breaking changes. Pin your version.

## License

Licensed under the [MIT License](LICENSE).
