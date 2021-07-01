#![allow(dead_code)]
mod machine;
mod machine2;

use machine2::{AccessKind, RefKind, TokenMachine};

fn main() {
    let (r1, mut machine) = TokenMachine::init();

    println!("{:?}", machine);
    let r2 = machine.create_ref(r1, RefKind::Unique);
    println!("{:?}", machine);
    let r3 = machine.create_ref(r1, RefKind::Unique);
    println!("{:?}", machine);
    machine.borrow_token(r2);
    println!("{:?}", machine);
    machine.use_token(r2, AccessKind::Write);
    println!("{:?}", machine);
    machine.return_token(r2);
    println!("{:?}", machine);
    machine.borrow_token(r3);
    println!("{:?}", machine);
    machine.use_token(r3, AccessKind::Write);
    println!("{:?}", machine);
    machine.return_token(r3);
    println!("{:?}", machine);
    machine.use_token(r1, AccessKind::Write);
    println!("{:?}", machine);
}
