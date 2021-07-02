use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefState {
    Created,
    Borrowing,
    Dead,
}

#[derive(Debug, Copy, Clone)]
pub struct RefInfo {
    parent: Reference,
    state: RefState,
}

#[derive(Debug, Clone)]
pub struct TokenMachine {
    ref_count: u32,
    current_owner: Reference,
    ref_info: HashMap<Reference, RefInfo>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Reference(u32);

impl TokenMachine {
    pub fn init() -> (Reference, Self) {
        let initial_ref = Reference(0);
        let mut ref_info = HashMap::new();
        ref_info.insert(
            initial_ref,
            RefInfo {
                // For simplicity, the initial reference has itself as its parent.
                // This means we don't require an Option to distinguish whether a
                // reference has a parent or not.
                parent: initial_ref,
                state: RefState::Borrowing,
            },
        );

        (
            initial_ref,
            TokenMachine {
                ref_count: 1,
                current_owner: initial_ref,
                ref_info,
            },
        )
    }

    pub fn create_ref(&mut self, parent: Reference) -> Reference {
        let id = self.ref_count;
        self.ref_count += 1;
        let new_ref = Reference(id);

        self.ref_info.insert(
            new_ref,
            RefInfo {
                state: RefState::Created,
                parent,
            },
        );

        new_ref
    }

    pub fn borrow_token(&mut self, target: Reference) {
        let target_info = self.ref_info[&target];
        match target_info.state {
            RefState::Created => {}
            RefState::Borrowing => panic!("Cannot create borrowing cycle"),
            RefState::Dead => panic!("Target cannot be dead"),
        };

        self.ref_info.get_mut(&target).unwrap().state = RefState::Borrowing;
        self.current_owner = target;
    }

    pub fn return_token(&mut self) {
        let current = self.current_owner;
        let current_info = self.ref_info[&current];
        let parent = current_info.parent;

        self.ref_info.get_mut(&current).unwrap().state = RefState::Dead;
        self.current_owner = parent;
    }

    pub fn use_token(&mut self, source: Reference) {
        if source != self.current_owner {
            panic!("You can only use the token if you have it");
        }
    }
}
