//! # Outbox-Complicated — poprako-orchestra with sqlx
//!
//! Demonstrates a usecase that runs a non-transactional check **before** the
//! transaction, then performs transactional steps inside a single nucleus
//! transaction:

//! 1. [`Run`] — check user existence and retrieve the current avatar OSS key.
//!    A `Run` needs no context and executes against the pool, outside any
//!    transaction; its result decides whether to begin one at all.
//! 2. [`Step`] — clear the `avatar_url` column inside the transaction.
//! 3. [`Step`] — insert an outbox event in the same transaction so a
//!    downstream consumer can clean up the OSS resource after commit.

use sqlx::{PgPool, Postgres, Transaction};

use poprako_orchestra::OperRun as _;
use poprako_orchestra::OperStep as _;
use poprako_orchestra::nucl::{Nucl, NuclError};
use poprako_orchestra::step::{Run, Step};
use poprako_orchestra::{AtLeast, Level, Oper, Context, drive};

// ---------------------------------------------------------------------------
// Domain — Oper definitions
// ---------------------------------------------------------------------------

pub struct ReadCommitted;

impl Level for ReadCommitted {}

pub struct RepeatableRead;

impl Level for RepeatableRead {}

pub struct Serializable;

impl Level for Serializable {}

impl AtLeast<RepeatableRead> for Serializable {}

/// Check whether the user exists and return the current avatar OSS key (if
/// any). A [`Run`] operation executes against the pool, outside any
/// transaction; its result decides whether the usecase opens a transaction.
///
/// `None`            — user not found (caller should short-circuit).
/// `Some(Some(key))` — user exists **and** has an avatar to clean up.
/// `Some(None)`      — user exists but has no avatar (skip OSS cleanup).
#[derive(Oper)]
#[oper(output = Option<String>)]
pub struct ExistAvatar<'a> {
    pub id: &'a str,
}

/// Clear the `avatar_url` column to `NULL`.  Executed **inside** the
/// transaction so any subsequent rollback restores the URL.
#[derive(Oper)]
#[oper(output = ())]
pub struct DeleteAvatar<'a> {
    pub id: &'a str,
}

/// Insert an outbox event so a downstream consumer can perform the actual
/// OSS resource cleanup.  Executed **inside** the same transaction as the
/// avatar deletion so the two stay atomic.
#[derive(Oper)]
#[oper(output = ())]
pub struct CleanOssImage<'a> {
    pub id: &'a str,
    pub key: &'a str,
}

// ---------------------------------------------------------------------------
// Domain — Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct RegularError(Box<dyn std::error::Error + Send>);

impl From<sqlx::Error> for RegularError {
    fn from(e: sqlx::Error) -> Self {
        RegularError(Box::new(e))
    }
}

impl std::fmt::Display for RegularError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for RegularError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

// ---------------------------------------------------------------------------
// Domain — Repo traits
// ---------------------------------------------------------------------------

#[drive(
    context = C,
    error = RegularError,
    run(for<'a> ExistAvatar<'a>),
    step(for<'a> DeleteAvatar<'a>),
)]
pub trait UserRepo<C> {}

#[drive(
    context = C,
    error = RegularError,
    step(for<'a> CleanOssImage<'a>),
)]
pub trait OutboxRepo<C> {}

// ---------------------------------------------------------------------------
// Infra — Context + Nucl
// ---------------------------------------------------------------------------

pub struct PgContext(Transaction<'static, Postgres>);

impl Context for PgContext {
    type Level = Serializable;
}

pub struct PgNucl(PgPool);

impl PgNucl {
    pub fn new(pool: PgPool) -> Self {
        Self(pool)
    }
}

impl Nucl for PgNucl {
    type Level = Serializable;
    type Error = sqlx::Error;
    type Context = PgContext;

    async fn coord<F, T, E>(&self, f: F) -> Result<T, NuclError<Self::Error, E>>
    where
        F: for<'cx> AsyncFnOnce(&'cx mut Self::Context) -> Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        let mut tx = self.0.begin().await.map_err(NuclError::Backend)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *tx)
            .await
            .map_err(NuclError::Backend)?;

        let mut cx = PgContext(tx);

        match f(&mut cx).await {
            Ok(value) => {
                cx.0.commit().await.map_err(NuclError::Backend)?;
                Ok(value)
            }
            Err(err) => {
                let _ = cx.0.rollback().await;
                Err(NuclError::Step(err))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Infra — Repos
// ---------------------------------------------------------------------------

pub struct UserRepoImpl {
    pool: PgPool,
}

impl Run<ExistAvatar<'_>> for UserRepoImpl {
    type Error = RegularError;

    async fn run(&self, oper: &ExistAvatar<'_>) -> Result<Option<String>, RegularError> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT avatar_url FROM users WHERE id = $1")
                .bind(oper.id)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some(inner) => Ok(inner), // user found → avatar_url (NULL → None, url → Some)
            None => Err(RegularError(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("user '{}' not found", oper.id),
            )))),
        }
    }
}

impl Step<DeleteAvatar<'_>, PgContext> for UserRepoImpl {
    type Level = RepeatableRead;
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &DeleteAvatar<'_>) -> Result<(), RegularError> {
        sqlx::query("UPDATE users SET avatar_url = NULL WHERE id = $1")
            .bind(oper.id)
            .execute(&mut *cx.0)
            .await?;

        Ok(())
    }
}

pub struct OutboxRepoImpl;

impl Step<CleanOssImage<'_>, PgContext> for OutboxRepoImpl {
    type Level = RepeatableRead;
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &CleanOssImage<'_>) -> Result<(), RegularError> {
        sqlx::query(
            "INSERT INTO outbox (event_type, user_id, oss_key) \
             VALUES ('avatar_deleted', $1, $2)",
        )
        .bind(oper.id)
        .bind(oper.key)
        .execute(&mut *cx.0)
        .await?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Usecase
// ---------------------------------------------------------------------------

async fn delete_avatar_usecase<C, N, R1, R2>(
    nucl: &N,
    user_repo: &R1,
    outbox_repo: &R2,
    id: &str,
    key: &str,
) -> Result<(), RegularError>
where
    C: Context + Send,
    C::Level: AtLeast<RepeatableRead>,
    N: Nucl<Context = C>,
    N::Error: std::error::Error + Send + 'static,
    R1: UserRepo<C> + Send + Sync,
    R2: OutboxRepo<C> + Send + Sync,
{
    // ── Non-transactional check: a `Run` needs no context and runs against
    //    the pool, outside any transaction. Its result decides whether we
    //    open a transaction at all. ──
    let avatar_key = ExistAvatar { id }.run_on(user_repo).await?;
    let Some(_key) = avatar_key.as_deref() else {
        return Ok(()); // nothing to clean up → no transaction needed
    };

    // ── Transactional steps: clear avatar_url + insert outbox entry ──
    match nucl
        .coord(async |cx| {
            DeleteAvatar { id }.step_on(user_repo, cx).await?;
            CleanOssImage { id, key }.step_on(outbox_repo, cx).await?;
            Ok(())
        })
        .await
    {
        Err(NuclError::Backend(e)) => Err(RegularError(Box::new(e))),
        Err(NuclError::Step(e)) => Err(e),
        Ok(()) => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost:5432/test".into());
    let pool = PgPool::connect(&database_url).await?;

    let nucl = PgNucl::new(pool.clone());
    let user_repo = UserRepoImpl { pool: pool.clone() };
    let outbox_repo = OutboxRepoImpl;

    let result =
        delete_avatar_usecase(&nucl, &user_repo, &outbox_repo, "user_1", "avatars/foo.jpg").await;

    match result {
        Ok(()) => println!("Avatar deleted and outbox event sent"),
        Err(e) => eprintln!("Failed: {}", e),
    }

    Ok(())
}
