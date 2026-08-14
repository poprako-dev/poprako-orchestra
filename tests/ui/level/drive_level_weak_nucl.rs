use poprako_orchestra::{AtLeast, Level, Oper , Context, Step, drive};

struct RepeatableRead;
impl Level for RepeatableRead {}

struct Serializable;
impl Level for Serializable {}
impl AtLeast<RepeatableRead> for Serializable {}

#[derive(Oper)]
#[oper(output = ())]
struct CreateUser;

#[drive(
    context = C,
    error = (),
    step(CreateUser),
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

// 流程要求上下文必须是 Serializable。
fn usecase<C, R>()
where
    C: Context<Level = Serializable>,
    R: UserRepo<C>,
{
}

// nucl 只提供可重复读。
struct RrCx;
impl Context for RrCx {
    type Level = RepeatableRead;
}

fn main() {
    usecase::<RrCx, Repo>();
}
