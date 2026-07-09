//! # SQLx-Basic — poprako-orchestra with sqlx

use poprako_orchestra::nucl::Nucl;
use poprako_orchestra::nucl::NuclError;
use poprako_orchestra::oper::Oper;
use poprako_orchestra::step::Step;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;

// ---------------------------------------------------------------------------
// Domain — Oper definitions
// ---------------------------------------------------------------------------

pub struct DecreaseProduct {
    pub product_id: i32,
    pub quantity: i32,
}

impl Oper for DecreaseProduct {
    type Output = ();
}

pub struct CreateOrder {
    pub user_id: i32,
    pub product_id: i32,
    pub quantity: i32,
}

impl Oper for CreateOrder {
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
// Infra — Repo
// ---------------------------------------------------------------------------

pub struct PgRepo;

impl Step<DecreaseProduct, PgContext> for PgRepo {
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &DecreaseProduct) -> Result<(), RegularError> {
        sqlx::query("UPDATE products SET stock = stock - $1 WHERE id = $2")
            .bind(oper.quantity)
            .bind(oper.product_id)
            .execute(&mut *cx.0)
            .await?;

        Ok(())
    }
}

impl Step<CreateOrder, PgContext> for PgRepo {
    type Error = RegularError;

    async fn step(&self, cx: &mut PgContext, oper: &CreateOrder) -> Result<(), RegularError> {
        sqlx::query("INSERT INTO orders (user_id, product_id, quantity) VALUES ($1, $2, $3)")
            .bind(oper.user_id)
            .bind(oper.product_id)
            .bind(oper.quantity)
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
    C: Send,
    N: Nucl<Context = C>,
    N::Error: std::error::Error + Send + 'static,
    R: Step<DecreaseProduct, C, Error = RegularError>
        + Step<CreateOrder, C, Error = RegularError>
        + Send
        + Sync,
{
    match nucl
        .coord(async |cx| {
            repo.step(
                cx,
                &DecreaseProduct {
                    product_id,
                    quantity,
                },
            )
            .await?;

            repo.step(
                cx,
                &CreateOrder {
                    user_id,
                    product_id,
                    quantity,
                },
            )
            .await?;

            Ok(())
        })
        .await
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
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost:5432/test".into());
    let pool = PgPool::connect(&database_url).await?;

    let nucl = PgNucl::new(pool);
    let repo = PgRepo;

    let result = run_order_usecase(&nucl, &repo, 1, 1, 1).await;

    match result {
        Ok(()) => println!("Transaction completed successfully"),
        Err(_) => eprintln!("Transaction failed"),
    }

    Ok(())
}
