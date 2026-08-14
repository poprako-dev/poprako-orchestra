use poprako_orchestra::{AtLeast, Level, Oper , Context, Step, drive};

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

#[drive(
    context = C,
    error = (),
    step(CreateUser, DeleteUser),
)]
trait UserRepo<C> {}

struct Repo;

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

// 一次声明切到串行化，无逐-op 级别约束。
async fn usecase<C, R>(cx: &mut C, repo: &R) -> Result<(), ()>
where
    C: Context<Level = Serializable>,
    R: UserRepo<C>,
{
    repo.step(cx, &CreateUser).await?;
    repo.step(cx, &DeleteUser).await
}

struct SerializableCx;
impl Context for SerializableCx {
    type Level = Serializable;
}

fn main() {
    fn require<C: Context, R: UserRepo<C>>() {}
    require::<SerializableCx, Repo>();

    let _ = usecase::<SerializableCx, Repo>;
}
