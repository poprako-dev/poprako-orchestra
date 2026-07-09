//! # Outbox-Complicated — poprako-orchestra with sqlx
//!
//! Demonstrates a mixed usecase that blends non-transactional checks with
//! transactional steps and post-commit side effects:
//!
//! 1. [`Run`] — check user existence and retrieve the current avatar OSS key
//!    (outside the transaction).
//! 2. [`Step`] — clear the `avatar_url` column inside a transaction.
//! 3. [`Run`] — insert an outbox event so a downstream consumer can clean up
//!    the OSS resource (after the transaction commits).

use poprako_orchestra::nucl::Nucl;
use poprako_orchestra::nucl::NuclError;
use poprako_orchestra::oper::Oper;
use poprako_orchestra::step::Run;
use poprako_orchestra::step::Step;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;

// ---------------------------------------------------------------------------
// Domain — Oper definitions
// ---------------------------------------------------------------------------

/// Check whether the user exists and return the current avatar OSS key (if
/// any).  Executed **outside** the transaction so a non-existent user is
/// caught before any writes begin.
pub struct ExistAvatar<'a> {
    pub id: &'a str,
}

impl Oper for ExistAvatar<'_> {
    /// `None`  — user not found (caller should short-circuit).
    /// `Some(Some(key))` — user exists **and** has an avatar to clean up.
    /// `Some(None)`      — user exists but has no avatar (skip OSS cleanup).
    type Output = Option<String>;
}

/// Clear the `avatar_url` column to `NULL`.  Executed **inside** the
/// transaction so any subsequent rollback restores the URL.
pub struct DeleteAvatar<'a> {
    pub id: &'a str,
}

impl Oper for DeleteAvatar<'_> {
    type Output = ();
}

/// Insert an outbox event so a downstream consumer can perform the actual
/// OSS resource cleanup.  Executed **inside** the same transaction as the
/// avatar deletion so the two stay atomic.
pub struct CleanOssImage<'a> {
    pub id: &'a str,
    pub key: &'a str,
}

impl Oper for CleanOssImage<'_> {
    type Output = ();
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
// Infra — Context + Nucl
// ---------------------------------------------------------------------------

pub struct PgContext(Transaction<'static, Postgres>);

pub struct PgNucl(PgPool);

impl PgNucl {
    pub fn new(pool: PgPool) -> Self {
        Self(pool)
    }
}

impl Nucl for PgNucl {
    type Error = sqlx::Error;
    type Context = PgContext;

    async fn coord<F, T, E>(&self, f: F) -> Result<T, NuclError<Self::Error, E>>
    where
        F: for<'cx> AsyncFnOnce(&'cx mut Self::Context) -> Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        let tx = self.0.begin().await.map_err(NuclError::Backend)?;

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

pub struct UserRepo {
    pool: PgPool,
}

impl Run<ExistAvatar<'_>> for UserRepo {
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

impl Step<DeleteAvatar<'_>, PgContext> for UserRepo {
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &DeleteAvatar<'_>) -> Result<(), RegularError> {
        sqlx::query("UPDATE users SET avatar_url = NULL WHERE id = $1")
            .bind(oper.id)
            .execute(&mut *cx.0)
            .await?;

        Ok(())
    }
}

pub struct OutboxRepo;

impl Step<CleanOssImage<'_>, PgContext> for OutboxRepo {
    type Error = RegularError;

    async fn step(
        &self,
        cx: &mut PgContext,
        oper: &CleanOssImage<'_>,
    ) -> Result<(), RegularError> {
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
    C: Send,
    N: Nucl<Context = C>,
    N::Error: std::error::Error + Send + 'static,
    R1: Send + Sync,
    R2: Send + Sync,
    for<'a> R1: Run<ExistAvatar<'a>, Error = RegularError>,
    for<'a> R1: Step<DeleteAvatar<'a>, C, Error = RegularError>,
    for<'a> R2: Step<CleanOssImage<'a>, C, Error = RegularError>,
{
    // ── Step 1: check existence + get avatar key (outside tx) ──
    let _ = user_repo.run(&ExistAvatar { id }).await?;

    // ── Step 2: clear avatar_url + insert outbox entry (inside tx) ──
    match nucl
        .coord(async |cx| {
            user_repo.step(cx, &DeleteAvatar { id }).await?;
            outbox_repo.step(cx, &CleanOssImage { id, key }).await?;
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
    let user_repo = UserRepo { pool: pool.clone() };
    let outbox_repo = OutboxRepo;

    let result =
        delete_avatar_usecase(&nucl, &user_repo, &outbox_repo, "user_1", "avatars/foo.jpg").await;

    match result {
        Ok(()) => println!("Avatar deleted and outbox event sent"),
        Err(e) => eprintln!("Failed: {}", e),
    }

    Ok(())
}
