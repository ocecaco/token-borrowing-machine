use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefState {
    // Active means the reference either has received the token already, but not
    // yet returned it to its parent (although it may have passed it along to a
    // child), or it has never received the token at all.
    Active,
    // Dead means that the reference has returned the token to its parent and
    // can never receive it again.
    Dead,
}

#[derive(Debug, Copy, Clone)]
pub struct RefInfo {
    // The reference this reference was derived from
    parent: Reference,
    // Current state of the reference
    state: RefState,
}

#[derive(Debug, Clone)]
pub struct TokenMachine {
    // A counter that is bumped to generate new reference IDs.
    ref_count: u32,
    // The reference that currently holds the token
    current_owner: Reference,
    ref_info: HashMap<Reference, RefInfo>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Reference(u32);

impl TokenMachine {
    // In the initial state of the machine, there is a single reference
    // (borrowing from itself) holding the token.
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
                state: RefState::Active,
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

    // Create a new reference with another reference as its parent. (The
    // reference can only initially receive the token from its parent)
    pub fn create_ref(&mut self, parent: Reference) -> Reference {
        let id = self.ref_count;
        self.ref_count += 1;
        let new_ref = Reference(id);

        self.ref_info.insert(
            new_ref,
            RefInfo {
                state: RefState::Active,
                parent,
            },
        );

        new_ref
    }

    // Lend the token from a parent to its child. The reference [target] is the
    // child and the token is borrowed from the parent.
    pub fn borrow_token(&mut self, target: Reference) {
        let target_info = self.ref_info[&target];
        let source = target_info.parent;

        // Parent needs to currently hold the token
        if self.current_owner != source {
            panic!("Parent needs to have the token in order to lend it to a child");
        }

        match target_info.state {
            RefState::Active => {}
            RefState::Dead => panic!("Target cannot be dead"),
        };

        self.current_owner = target;
    }

    // Return the token from the child (the current owner) to its parent. This
    // causes the child to die, meaning it can never receive the token again.
    pub fn return_token(&mut self) {
        let source = self.current_owner;
        let source_info = self.ref_info[&source];
        let target = source_info.parent;

        // You die when you return the token, meaning you can never receive it
        // again.
        self.ref_info.get_mut(&source).unwrap().state = RefState::Dead;
        self.current_owner = target;
    }

    // Use the token to perform a memory access. This requires the reference
    // [source] to be the current owner of the token.
    pub fn use_token(&mut self, source: Reference) {
        if source != self.current_owner {
            panic!("You can only use the token if you have it");
        }
    }
}
