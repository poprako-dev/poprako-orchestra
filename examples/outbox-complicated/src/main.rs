//! # Outbox-Complicated — poprako-orchestra with sqlx

use poprako_orchestra::nucl::Nucl;
use poprako_orchestra::nucl::NuclError;
use poprako_orchestra::oper::Oper;
use poprako_orchestra::step::Step;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;

pub struct Exist<'a> {
    pub id: &'a str,
}

pub struct DeleteAvatar<'a> {
    pub id: &'a str,
}

pub struct UserRepo {
    pool: PgPool,
}

pub struct CleanOssImage<'a> {
    pub id: &'a str,
    pub key: &'a str,
}

pub struct OutboxRepo;

pub struct PgNucl(PgPool);

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

#[tokio::main]
async fn main() {
    todo!()
}
