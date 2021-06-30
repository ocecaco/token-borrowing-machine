#![allow(dead_code)]
mod machine;
mod machine2;

use machine2::{AccessKind, InteriorMut, TokenMachine, TokenState};

fn main() {
    let (r1, mut machine) = TokenMachine::init();

    println!("{:?}", machine);
    let r2 = machine.create_ref();
    println!("{:?}", machine);
    let r3 = machine.create_ref();
    println!("{:?}", machine);
    machine.lend_token(r1, r2);
    println!("{:?}", machine);
    machine.use_token(r2, AccessKind::Write, InteriorMut::Default);
    println!("{:?}", machine);
    machine.return_token(r2);
    println!("{:?}", machine);
    machine.lend_token(r1, r3);
    println!("{:?}", machine);
    machine.use_token(r3, AccessKind::Write, InteriorMut::Default);
    println!("{:?}", machine);
    machine.return_token(r3);
    println!("{:?}", machine);
    machine.set_token_state(TokenState::SharedReadOnly);
    println!("{:?}", machine);
    machine.dup_token(r1);
    machine.dup_token(r1);
    machine.dup_token(r1);
    println!("{:?}", machine);
    let r4 = machine.create_ref();
    let r5 = machine.create_ref();
    machine.lend_token(r1, r4);
    machine.lend_token(r1, r5);
    machine.use_token(r1, AccessKind::Read, InteriorMut::Default);
    machine.use_token(r4, AccessKind::Read, InteriorMut::Default);
    machine.use_token(r5, AccessKind::Read, InteriorMut::Default);
    println!("{:?}", machine);
}
