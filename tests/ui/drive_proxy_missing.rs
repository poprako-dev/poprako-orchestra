use poprako_orchestra::drive;

#[derive(poprako_orchestra::Oper)]
#[oper(output = ())]
struct CreateUser;

// `proxy` 需要至少一个 run/step 操作。
#[drive(
    context = C,
    error = (),
    proxy = UserRepoProxy,
)]
trait UserRepo<C> {}

fn main() {}
