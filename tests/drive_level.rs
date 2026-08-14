#![cfg(feature = "macro")]

use poprako_orchestra::{AtLeast, Context, Level, Oper, Step, drive};

struct RepeatableRead;
impl Level for RepeatableRead {}

struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

#[derive(Oper)]
#[oper(output = ())]
struct CreateUser;

#[derive(Oper)]
#[oper(output = ())]
struct DeleteUser;

/// The capability aggregates operations only. Each `Step::Level` stays owned
/// by its (stepper, oper) pair; `#[drive]` propagates every step requirement.
#[drive(
    context = C,
    error = (),
    step(CreateUser, DeleteUser),
)]
trait UserRepo<C> {}

struct Repo;

// One repo, two operations, two different levels.
impl<C: Context + Send> Step<CreateUser, C> for Repo {
    type Level = RepeatableRead;
    type Error = ();

    async fn step(&self, _cx: &mut C, _oper: &CreateUser) -> Result<(), ()> {
        Ok(())
    }
}

impl<C: Context + Send> Step<DeleteUser, C> for Repo {
    type Level = Serializable;
    type Error = ();

    async fn step(&self, _cx: &mut C, _oper: &DeleteUser) -> Result<(), ()> {
        Ok(())
    }
}

struct SerializableCx;
impl Context for SerializableCx {
    type Level = Serializable;
}

/// 普通事务：只声明一次业务最低级别，没有任何逐-op 级别约束。
async fn create_user_usecase<C, R>(cx: &mut C, repo: &R) -> Result<(), ()>
where
    C: Context,
    C::Level: AtLeast<RepeatableRead>,
    R: UserRepo<C>,
{
    repo.step(cx, &CreateUser).await
}

/// 五个流程之一：一次声明直接切到串行化，同样没有逐-op 约束。
async fn purge_usecase<C, R>(cx: &mut C, repo: &R) -> Result<(), ()>
where
    C: Context<Level = Serializable>,
    R: UserRepo<C>,
{
    repo.step(cx, &CreateUser).await?;
    repo.step(cx, &DeleteUser).await
}

#[test]
fn drive_propagates_per_step_levels_without_usecase_leaks() {
    fn require_repo<C: Context, R: UserRepo<C>>() {}
    require_repo::<SerializableCx, Repo>();

    let _ = create_user_usecase::<SerializableCx, Repo>;
    let _ = purge_usecase::<SerializableCx, Repo>;
}
