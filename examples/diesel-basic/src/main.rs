//! # Diesel-Basic — poprako-orchestra with diesel_async

use diesel_async::RunQueryDsl as _;
use diesel_async::TransactionManager as _;
use diesel_async::{AnsiTransactionManager, AsyncPgConnection};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::{Object, Pool};

use poprako_orchestra::OperStep as _;
use poprako_orchestra::{AtLeast, Level, Oper, Scope, drive};
use poprako_orchestra::nucl::{Nucl, NuclError};
use poprako_orchestra::step::Step;

// ---------------------------------------------------------------------------
// Domain — Oper definitions
// ---------------------------------------------------------------------------

pub struct RepeatableRead;

impl Level for RepeatableRead {}

pub struct Serializable;

impl Level for Serializable {}

impl AtLeast<RepeatableRead> for Serializable {}

#[derive(Oper)]
#[oper(output = (), level = RepeatableRead)]
pub struct DecreaseProduct {
    pub product_id: i32,
    pub quantity: i32,
}

#[derive(Oper)]
#[oper(level = RepeatableRead, output = ())]
pub struct CreateOrder {
    pub user_id: i32,
    pub product_id: i32,
    pub quantity: i32,
}

// ---------------------------------------------------------------------------
// Domain — Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct RegularError(Box<dyn std::error::Error + Send>);

impl From<diesel::result::Error> for RegularError {
    fn from(e: diesel::result::Error) -> Self {
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
// Domain — Repo trait
// ---------------------------------------------------------------------------

#[drive(
    context = C,
    error = RegularError,
    step(DecreaseProduct, CreateOrder),
)]
pub trait OrderRepo<C> {}

// ---------------------------------------------------------------------------
// Infra — Context + Nucl
// ---------------------------------------------------------------------------

type Conn = Object<AsyncPgConnection>;

pub struct PgContext(Conn);

impl Scope for PgContext {
    type Level = Serializable;
}

#[derive(Debug)]
pub enum PgBackendError {
    Pool(String),
    Diesel(diesel::result::Error),
}

impl std::fmt::Display for PgBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PgBackendError::Pool(msg) => write!(f, "pool error: {}", msg),
            PgBackendError::Diesel(e) => write!(f, "diesel error: {}", e),
        }
    }
}

impl std::error::Error for PgBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PgBackendError::Pool(_) => None,
            PgBackendError::Diesel(e) => Some(e),
        }
    }
}

impl From<diesel::result::Error> for PgBackendError {
    fn from(e: diesel::result::Error) -> Self {
        PgBackendError::Diesel(e)
    }
}

pub struct PgNucl(Pool<AsyncPgConnection>);

impl PgNucl {
    pub fn new(pool: Pool<AsyncPgConnection>) -> Self {
        Self(pool)
    }
}

impl Nucl for PgNucl {
    type Level = Serializable;
    type Error = PgBackendError;
    type Context = PgContext;

    async fn coord<F, T, E>(&self, f: F) -> Result<T, NuclError<Self::Error, E>>
    where
        F: for<'cx> AsyncFnOnce(&'cx mut Self::Context) -> Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        let mut conn = self.0.get().await
            .map_err(|e| NuclError::Backend(PgBackendError::Pool(e.to_string())))?;

        AnsiTransactionManager::begin_transaction(&mut *conn)
            .await
            .map_err(|e| NuclError::Backend(PgBackendError::Diesel(e)))?;

        diesel::sql_query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *conn)
            .await
            .map_err(|e| NuclError::Backend(PgBackendError::Diesel(e)))?;

        let mut cx = PgContext(conn);

        let ret = f(&mut cx).await;

        match ret {
            Ok(value) => {
                AnsiTransactionManager::commit_transaction(&mut *cx.0)
                    .await
                    .map_err(|e| NuclError::Backend(PgBackendError::Diesel(e)))?;
                Ok(value)
            }
            Err(err) => {
                let _ = AnsiTransactionManager::rollback_transaction(&mut *cx.0).await;
                Err(NuclError::Step(err))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Infra — Repo
// ---------------------------------------------------------------------------

pub struct PgRepo;

impl PgRepo {
    pub fn new() -> Self {
        Self
    }
}

impl Step<DecreaseProduct, PgContext> for PgRepo {
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &DecreaseProduct) -> Result<(), RegularError> {
        diesel::sql_query("UPDATE products SET stock = stock - $1 WHERE id = $2")
            .bind::<diesel::sql_types::Integer, _>(oper.quantity)
            .bind::<diesel::sql_types::Integer, _>(oper.product_id)
            .execute(&mut *cx.0)
            .await?;
        Ok(())
    }
}

impl Step<CreateOrder, PgContext> for PgRepo {
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &CreateOrder) -> Result<(), RegularError> {
        diesel::sql_query("INSERT INTO orders (user_id, product_id, quantity) VALUES ($1, $2, $3)")
            .bind::<diesel::sql_types::Integer, _>(oper.user_id)
            .bind::<diesel::sql_types::Integer, _>(oper.product_id)
            .bind::<diesel::sql_types::Integer, _>(oper.quantity)
            .execute(&mut *cx.0)
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Usecase
// ---------------------------------------------------------------------------

async fn run_order_usecase<C, N, R>(
    nucl: &N,
    repo: &R,
    product_id: i32,
    user_id: i32,
    quantity: i32,
) -> Result<(), RegularError>
where
    C: Scope + Send,
    C::Level: AtLeast<RepeatableRead>,
    N: Nucl<Context = C>,
    N::Error: std::error::Error + Send + 'static,
    R: OrderRepo<C> + Send + Sync,
{
    match nucl.coord(async |cx| {
        DecreaseProduct {
            product_id,
            quantity,
        }
        .step_on(repo, cx)
        .await?;
        CreateOrder {
            user_id,
            product_id,
            quantity,
        }
        .step_on(repo, cx)
        .await?;
        Ok(())
    }).await
    {
        Ok(()) => Ok(()),
        Err(NuclError::Backend(e)) => Err(RegularError(Box::new(e))),
        Err(NuclError::Step(e)) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost:5432/test".into());

    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    let pool = Pool::builder(config).build()?;

    let nucl = PgNucl::new(pool);
    let repo = PgRepo::new();

    let result = run_order_usecase(&nucl, &repo, 1, 1, 1).await;

    match result {
        Ok(()) => println!("Transaction completed successfully"),
        Err(_) => eprintln!("Transaction failed"),
    }

    Ok(())
}
