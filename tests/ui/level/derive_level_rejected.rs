use poprako_orchestra::Oper;

struct Transactional;

#[derive(Oper)]
#[oper(output = (), level = Transactional)]
struct LeakedTransactionSemantics;

fn main() {}
