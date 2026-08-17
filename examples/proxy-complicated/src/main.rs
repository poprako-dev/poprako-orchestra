//! # Proxy-Complicated — standard capabilities with priority routing
//!
//! One mixed repository provides both user and comic capabilities, while an
//! independent publisher provides the promotion capability. `LoadComic`
//! deliberately supports both `Run` and `Step`; the default `step > run`
//! priority selects the transactional implementation without leaking that
//! choice into [`OrderComplex::place`].

use poprako_orchestra::OperProxy as _;
use poprako_orchestra::{Context, Level, Oper, Run, Step, drive};

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

#[drive(
    context = Cx,
    error = String,
    proxy = UserRepoProxy,
    run(for<'a> EnsureUser<'a>, for<'a> LoadComic<'a>),
    step(for<'a> LoadComic<'a>),
)]
pub trait UserRepo {}

#[drive(
    context = Cx,
    error = String,
    proxy = ComicRepoProxy,
    run(for<'comic> LoadComic<'comic>),
    step(
        for<'comic> LoadComic<'comic>,
        for<'order> CreateOrder<'order>
    ),
)]
pub trait ComicRepo {}

#[drive(
    context = Cx,
    error = String,
    proxy = PromProxy,
    step(for<'a> RecordPromotion<'a>),
)]
pub trait Prom {}

pub struct OrderComplex;

impl OrderComplex {
    pub async fn place<P>(
        proxy: &mut P,
        user_id: &str,
        comic_id: &str,
    ) -> Result<u64, String>
    where
        P: UserRepoProxy + ComicRepoProxy + PromProxy,
    {
        EnsureUser { user_id }.proxy_on(proxy).await?;
        let title = LoadComic { comic_id }.proxy_on(proxy).await?;
        RecordPromotion { comic_id }.proxy_on(proxy).await?;
        let order_id = CreateOrder { user_id, comic_id }.proxy_on(proxy).await?;
        println!("placing order for {title}");
        Ok(order_id)
    }
}

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

impl Step<LoadComic<'_>, Cx> for MixedRepo {
    type Level = Linearizable;
    type Error = String;

    async fn step(
        &self,
        context: &mut Cx,
        oper: &LoadComic<'_>,
    ) -> Result<String, Self::Error> {
        context.events.push(format!("load {}", oper.comic_id));
        Ok(format!("step:{}", oper.comic_id))
    }
}

impl Step<CreateOrder<'_>, Cx> for MixedRepo {
    type Level = Linearizable;
    type Error = String;

    async fn step(
        &self,
        context: &mut Cx,
        oper: &CreateOrder<'_>,
    ) -> Result<u64, Self::Error> {
        context.events.push(format!(
            "order {} for {}",
            oper.comic_id, oper.user_id,
        ));
        Ok(1)
    }
}

pub struct PromImpl;

impl Step<RecordPromotion<'_>, Cx> for PromImpl {
    type Level = Linearizable;
    type Error = String;

    async fn step(
        &self,
        context: &mut Cx,
        oper: &RecordPromotion<'_>,
    ) -> Result<(), Self::Error> {
        context.events.push(format!("promote {}", oper.comic_id));
        Ok(())
    }
}

async fn place_order(
    context: &mut Cx,
    repo: &MixedRepo,
    prom: &PromImpl,
) -> Result<u64, String> {
    let mut proxy = poprako_orchestra::proxy! {
        run => repo as UserRepoProxy + ComicRepoProxy;
        step(context) =>
            repo as UserRepoProxy + ComicRepoProxy,
            prom as PromProxy;
    };

    OrderComplex::place(&mut proxy, "u1", "c1").await
}

fn main() {
    futures::executor::block_on(async {
        let mut context = Cx { events: Vec::new() };
        let order_id = place_order(&mut context, &MixedRepo, &PromImpl)
            .await
            .expect("capability proxy should place the order");

        assert_eq!(order_id, 1);
        assert_eq!(
            context.events,
            ["load c1", "promote c1", "order c1 for u1"],
        );
    });
}
