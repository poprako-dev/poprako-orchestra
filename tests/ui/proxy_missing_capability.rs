use poprako_orchestra::proxy;

struct Repo;

fn main() {
    let repo = &Repo;
    let _proxy = proxy! {
        run => repo as MissingProxy;
    };
}
