#![allow(dead_code)]
mod machine;
mod machine2;

use machine2::{AccessKind, InteriorMut, TokenMachine, TokenState};

fn main() {
    let (r1, mut machine) = TokenMachine::init();

    println!("{:?}", machine);
    let r2 = machine.create_ref(r1);
    println!("{:?}", machine);
    let r3 = machine.create_ref(r1);
    println!("{:?}", machine);
    machine.borrow_token(r2);
    println!("{:?}", machine);
    machine.use_token(r2, AccessKind::Write, InteriorMut::Default);
    println!("{:?}", machine);
    machine.return_token(r2);
    println!("{:?}", machine);
    machine.borrow_token(r3);
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
    let r4 = machine.create_ref(r1);
    let r5 = machine.create_ref(r1);
    machine.borrow_token(r4);
    machine.borrow_token(r5);
    machine.use_token(r1, AccessKind::Read, InteriorMut::Default);
    machine.use_token(r4, AccessKind::Read, InteriorMut::Default);
    machine.use_token(r5, AccessKind::Read, InteriorMut::Default);
    println!("{:?}", machine);
    machine.return_token(r4);
    machine.return_token(r5);
    println!("{:?}", machine);
    machine.merge_token(r1);
    machine.merge_token(r1);
    machine.merge_token(r1);
    println!("{:?}", machine);
    machine.set_token_state(TokenState::Exclusive);
    let r6 = machine.create_ref(r1);
    machine.borrow_token(r6);
    machine.use_token(r6, AccessKind::Write, InteriorMut::Default);
    println!("{:?}", machine);
}
