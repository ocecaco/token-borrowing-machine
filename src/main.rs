mod machine;

use machine::TokenMachine;

fn main() {
    let (r1, mut machine) = TokenMachine::init();

    println!("{:?}", machine);
    machine.use_token(r1);
    println!("{:?}", machine);

    let r2 = machine.create_ref();
    let r3 = machine.create_ref();
    let r4 = machine.create_ref();

    println!("{:?}", machine);
    machine.lend_token(r2);
    println!("{:?}", machine);
    machine.use_token(r2);
    println!("{:?}", machine);
    machine.lend_token(r3);
    println!("{:?}", machine);
    machine.use_token(r3);
    println!("{:?}", machine);
    machine.lend_token(r4);
    println!("{:?}", machine);
    machine.return_token();
    println!("{:?}", machine);
    machine.use_token(r3);
    println!("{:?}", machine);
    machine.return_token();
    println!("{:?}", machine);
    machine.use_token(r2);
    println!("{:?}", machine);
    machine.return_token();
    println!("{:?}", machine);
    machine.use_token(r1);
    println!("{:?}", machine);
}
