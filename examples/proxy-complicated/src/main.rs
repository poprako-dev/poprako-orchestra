//! Explicit run and step proxies with operation-level bounds.

use poprako_orchestra::OperProxy as _;
use poprako_orchestra::{Context, Level, Oper, Proxy, Run, Step, drive};

pub struct Linearizable;
impl Level for Linearizable {}

pub struct Cx {
    pub events: Vec<String>,
}
impl Context for Cx {
    type Level = Linearizable;
}

#[derive(Oper)]
#[oper(output = ())]
pub struct EnsureUser<'a> {
    pub user_id: &'a str,
}

#[derive(Oper)]
#[oper(output = String)]
pub struct LoadComic<'a> {
    pub comic_id: &'a str,
}

#[derive(Oper)]
#[oper(output = u64)]
pub struct CreateOrder<'a> {
    pub user_id: &'a str,
    pub comic_id: &'a str,
}

#[derive(Oper)]
#[oper(output = ())]
pub struct RecordPromotion<'a> {
    pub comic_id: &'a str,
}

#[drive(error = String, run(for<'a> EnsureUser<'a>, for<'a> LoadComic<'a>))]
pub trait UserRepo {}

#[drive(
    context = Cx,
    error = String,
    step(for<'a> CreateOrder<'a>, for<'a> RecordPromotion<'a>),
)]
pub trait OrderRepo {}

pub struct MixedRepo;

impl Run<EnsureUser<'_>> for MixedRepo {
    type Error = String;

    async fn run(&self, oper: &EnsureUser<'_>) -> Result<(), Self::Error> {
        if oper.user_id.is_empty() {
            return Err("user ID must not be empty".to_owned());
        }
        Ok(())
    }
}

impl Run<LoadComic<'_>> for MixedRepo {
    type Error = String;

    async fn run(&self, oper: &LoadComic<'_>) -> Result<String, Self::Error> {
        Ok(format!("run:{}", oper.comic_id))
    }
}

impl Step<CreateOrder<'_>, Cx> for MixedRepo {
    type Level = Linearizable;
    type Error = String;

    async fn step(&self, context: &mut Cx, oper: &CreateOrder<'_>) -> Result<u64, Self::Error> {
        context.events.push(format!("order {} for {}", oper.comic_id, oper.user_id));
        Ok(1)
    }
}

pub struct PromImpl;
impl Step<RecordPromotion<'_>, Cx> for PromImpl {
    type Level = Linearizable;
    type Error = String;

    async fn step(&self, context: &mut Cx, oper: &RecordPromotion<'_>) -> Result<(), Self::Error> {
        context.events.push(format!("promote {}", oper.comic_id));
        Ok(())
    }
}

async fn run_order<P>(proxy: &mut P, user_id: &str, comic_id: &str) -> Result<String, String>
where
    P: for<'a> Proxy<EnsureUser<'a>, Error = String>
        + for<'a> Proxy<LoadComic<'a>, Error = String>,
{
    EnsureUser { user_id }.proxy_on(proxy).await?;
    LoadComic { comic_id }.proxy_on(proxy).await
}

async fn step_order<P>(proxy: &mut P, user_id: &str, comic_id: &str) -> Result<u64, String>
where
    P: for<'a> Proxy<CreateOrder<'a>, Error = String>
        + for<'a> Proxy<RecordPromotion<'a>, Error = String>,
{
    RecordPromotion { comic_id }.proxy_on(proxy).await?;
    CreateOrder { user_id, comic_id }.proxy_on(proxy).await
}

fn main() {
    futures::executor::block_on(async {
        let repo = &MixedRepo;
        let mut run_adapter = poprako_orchestra::proxy! {
            run {
                repo => for<'a> EnsureUser<'a>, for<'a> LoadComic<'a>;
            }
        };
        assert_eq!(run_order(&mut run_adapter, "u1", "c1").await.unwrap(), "run:c1");

        let prom = &PromImpl;
        let context = &mut Cx { events: Vec::new() };
        let mut step_adapter = poprako_orchestra::proxy! {
            step(context) {
                repo => for<'a> CreateOrder<'a>;
                prom => for<'a> RecordPromotion<'a>;
            }
        };
        assert_eq!(step_order(&mut step_adapter, "u1", "c1").await.unwrap(), 1);
        assert_eq!(context.events, ["promote c1", "order c1 for u1"]);
    });
}
