use poprako_orchestra::{Level, Oper};

struct Transactional;
impl Level for Transactional {}

#[derive(Oper)]
#[oper(output = (), level = Transactional, level = Transactional)]
struct DuplicateLevel;

fn main() {}
