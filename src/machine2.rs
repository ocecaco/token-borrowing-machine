use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefState {
    // Created means you've never held any tokens.
    Created,
    // This state means you have at some point received tokens (although you may
    // have lended them out again to someone else).
    TokensBorrowedFrom(Reference),
    // Dead means you gave back all your tokens: now you can never receive any
    // tokens again.
    Dead(Reference),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TokenState {
    // The token can be shared, but only read-only accesses are allowed
    SharedReadOnly,
    // The token can be shared, but only UnsafeCell accesses (reads/writes are
    // allowed). In particular, reading through an ordinary shared reference is
    // not allowed in this state, because that would probably mess up
    // optimizations. However, you don't have to be in this state to read
    // through an UnsafeCell: you can also do that in SharedReadOnly, and hence
    // you should stay there until you actually need to write.
    SharedReadWrite,
    // The token is exclusive, and can be used for any type of access.
    Exclusive,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InteriorMut {
    UnsafeCellOrRaw,
    Default,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub struct TokenMachine {
    ref_count: u32,
    // Invariant: if token_state is Exclusive, then the total number of tokens
    // in owners (sum of all values) should be 1.
    token_state: TokenState,
    // For each reference, how many tokens they have.
    num_tokens: HashMap<Reference, u32>,
    // Invariant: token_count should be equal to the sum of all values in
    // num_tokens.
    token_count: u32,
    ref_state: HashMap<Reference, RefState>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Reference(u32);

impl TokenMachine {
    pub fn init() -> (Reference, Self) {
        let initial_ref = Reference(0);

        let mut ref_state = HashMap::new();
        // Initial reference borrows from itself: this simplifies the code since
        // we don't have to consider two cases, one where a reference has a
        // parent and one where it doesn't.
        ref_state.insert(initial_ref, RefState::TokensBorrowedFrom(initial_ref));

        let mut num_tokens = HashMap::new();
        num_tokens.insert(initial_ref, 1);

        (
            initial_ref,
            TokenMachine {
                ref_count: initial_ref.0 + 1,
                token_state: TokenState::Exclusive,
                token_count: 1,
                num_tokens,
                ref_state,
            },
        )
    }

    pub fn create_ref(&mut self) -> Reference {
        let id = self.ref_count;
        self.ref_count += 1;

        let new_ref = Reference(id);
        self.ref_state.insert(new_ref, RefState::Created);
        self.num_tokens.insert(new_ref, 0);

        new_ref
    }

    pub fn lend_token(&mut self, source: Reference, target: Reference) {
        // You must own a token to lend one out
        if self.num_tokens[&source] == 0 {
            panic!("Need to have a token to lend one out");
        }

        // Target must be ready to receive a token.
        let target_state = self.ref_state[&target];
        match target_state {
            RefState::Created => {}
            RefState::TokensBorrowedFrom(source_old) => {
                if source != source_old {
                    panic!("Cannot lend to someone who is already lending from someone else")
                }
            }
            RefState::Dead { .. } => panic!("Target cannot be dead"),
        };

        // Transfer token from source to target and register where the target
        // got the token.
        *self.num_tokens.entry(source).or_insert(0) -= 1;
        *self.num_tokens.entry(target).or_insert(0) += 1;
        self.ref_state
            .insert(target, RefState::TokensBorrowedFrom(source));
    }

    pub fn return_token(&mut self, source: Reference) {
        if self.num_tokens[&source] == 0 {
            panic!("Cannot give back a token if you don't have one");
        }

        let target = match self.ref_state[&source] {
            RefState::Created => panic!("invariant violation"),
            RefState::TokensBorrowedFrom(target) => target,
            RefState::Dead(target) => target,
        };

        *self.num_tokens.entry(source).or_insert(0) -= 1;
        *self.num_tokens.entry(target).or_insert(0) += 1;

        // If you've given back all your tokens, you become dead, and cannot
        // receive new tokens. However, you can still pass along tokens from
        // your children, even if you are dead. So it is possible that
        // num_tokens > 0 in this state.
        if self.num_tokens[&source] == 0 {
            self.ref_state.insert(source, RefState::Dead(target));
        }
    }

    // TODO: Token duplication and state conversion (you can convert to
    // Exclusive if you hold all the tokens, and you can always go from
    // Exclusive to SharedRW or SharedRO).
    pub fn dup_token(&mut self, source: Reference) {
        if self.num_tokens[&source] == 0 {
            panic!("Cannot duplicate a token if you do not have a token");
        }

        if self.token_state == TokenState::Exclusive {
            panic!("Cannot duplicate exclusive token");
        }

        *self.num_tokens.entry(source).or_insert(0) += 1;
        self.token_count += 1;
    }

    pub fn merge_token(&mut self, source: Reference) {
        if self.num_tokens[&source] <= 1 {
            panic!("Can only merge tokens if you have more than one");
        }

        *self.num_tokens.entry(source).or_insert(0) -= 1;
        self.token_count -= 1;
    }

    pub fn set_token_state(&mut self, token_state: TokenState) {
        // TODO: Maybe this could be less strict, where we allow some
        // transitions, like SharedRW to SharedRO even if the token is not
        // exclusively owned?
        if self.token_count != 1 {
            panic!("Token cannot be split when changing state");
        }

        self.token_state = token_state;
    }

    pub fn use_token(
        &mut self,
        source: Reference,
        access_kind: AccessKind,
        interior_mut: InteriorMut,
    ) {
        if self.num_tokens[&source] == 0 {
            panic!("Cannot perform accesses without a token");
        }

        if let RefState::Dead { .. } = self.ref_state[&source] {
            panic!("Cannot read/write with a dead reference");
        }

        match self.token_state {
            // You can do any kind of read/write if the token is exclusively
            // owned.
            TokenState::Exclusive => {}
            TokenState::SharedReadOnly => {
                if access_kind != AccessKind::Read {
                    panic!("Cannot write with SharedRO token");
                }
            }
            TokenState::SharedReadWrite => {
                if interior_mut != InteriorMut::UnsafeCellOrRaw {
                    panic!("Can only do UnsafeCell/Raw access with SharedRW token");
                }
            }
        }
    }
}
