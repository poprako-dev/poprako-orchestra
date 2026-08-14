use poprako_orchestra::{AtLeast, Level, Oper , Context, Step, drive};

struct RepeatableRead;
impl Level for RepeatableRead {}

struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

struct Linearizable;
impl Level for Linearizable {}
// 没有 `Serializable: AtLeast<Linearizable>`。

#[derive(Oper)]
#[oper(output = ())]
struct DeleteUser;

#[drive(
    context = C,
    error = (),
    step(DeleteUser),
)]
trait UserRepo<C> {}

struct Repo;

impl<C: Context + Send> Step<DeleteUser, C> for Repo {
    // 这个 oper 的 step 要求超过流程声明的 Serializable。
    type Level = Linearizable;
    type Error = ();

    async fn step(&self, _cx: &mut C, _oper: &DeleteUser) -> Result<(), ()> {
        Ok(())
    }
}

fn usecase<C, R>()
where
    C: Context<Level = Serializable>,
    R: UserRepo<C>,
{
}

struct SerializableCx;
impl Context for SerializableCx {
    type Level = Serializable;
}

fn main() {
    usecase::<SerializableCx, Repo>();
}
