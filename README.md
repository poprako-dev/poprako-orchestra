# poprako-orchestra

A unified transaction abstraction framework for Rust — zero dependencies, pure `std`.

## Overview

`poprako-orchestra` decouples transactional logic into three independent concerns:

| Trait  | Role                        | Module              |
|--------|-----------------------------| ------------------- |
| `Oper` | **What** — the operation's input and output type | [`oper`] |
| `Step` | **How** — async executor running inside a transaction | [`step`] |
| `Nucl` | **Where** — the transactional backend (connection pool, saga, …) | [`nucl`] |

Plus a non-transactional variant:

| Trait  | Role                        | Module              |
|--------|-----------------------------| ------------------- |
| `Run`  | **How (no tx)** — async executor that runs without a managed context | [`step`] |

## Quick example

```rust
use poprako_orchestra::oper::Oper;
use poprako_orchestra::step::Step;
use poprako_orchestra::nucl::{Nucl, NuclError};

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
fn create_user(nucl: &impl Nucl<Context = DbConn>, repo: &UserRepo, name: String) {
    let result = nucl.coord(async |cx| {
        repo.step(cx, &CreateUser { name }).await
    }).await; // Result<u64, NuclError<db::Error, db::Error>>
}
```

## Why separate Oper from Step?

- The same `Oper` can be executed by different `Step` implementations in different contexts.
- `Step` is a stateless executor — all per-call state lives in the `Oper` value.
- This is the **Command pattern**: `Oper` is the command object, `Step` is the handler.

## Version policy

- Rust edition **2024** — requires Rust 1.85+.
- Pre-1.0: minor versions may include breaking changes. Pin your version.

## License

Licensed under the [MIT License](LICENSE).
