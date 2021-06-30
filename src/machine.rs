use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefState {
    Created,
    TokenBorrowedFrom(Reference),
    Dead,
}

#[derive(Debug, Clone)]
pub struct TokenMachine {
    ref_count: u32,
    current_owner: Reference,
    ref_state: HashMap<Reference, RefState>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Reference(u32);

impl TokenMachine {
    pub fn init() -> (Reference, Self) {
        let initial_ref = Reference(0);
        let mut initial_state = HashMap::new();
        initial_state.insert(initial_ref, RefState::TokenBorrowedFrom(initial_ref));
        (
            initial_ref,
            TokenMachine {
                ref_count: initial_ref.0 + 1,
                current_owner: initial_ref,
                ref_state: initial_state,
            },
        )
    }

    pub fn create_ref(&mut self) -> Reference {
        let id = self.ref_count;
        self.ref_count += 1;

        let new_ref = Reference(id);
        self.ref_state.insert(new_ref, RefState::Created);

        new_ref
    }

    pub fn lend_token(&mut self, target: Reference) {
        let target_state = self.ref_state[&target];
        match target_state {
            RefState::Created => {}
            RefState::TokenBorrowedFrom { .. } => panic!("Cannot create borrowing cycle"),
            RefState::Dead => panic!("Target cannot be dead"),
        };

        self.ref_state
            .insert(target, RefState::TokenBorrowedFrom(self.current_owner));
        self.current_owner = target;
    }

    pub fn return_token(&mut self) {
        let current_owner = self.current_owner;
        let original_owner =
            if let RefState::TokenBorrowedFrom(source) = self.ref_state[&current_owner] {
                source
            } else {
                panic!("Invariant violation: reference has token without borrowing it from anyone")
            };

        self.ref_state.insert(current_owner, RefState::Dead);
        self.current_owner = original_owner;
    }

    pub fn use_token(&mut self, source: Reference) {
        if source != self.current_owner {
            panic!("You can only use the token if you have it");
        }
    }
}
