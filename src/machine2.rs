use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefState {
    // Created means you've never held any tokens.
    Created,
    // This state means you have at some point received tokens (although you may
    // have lended them out again to someone else).
    Borrowing,
    // Dead means you gave back all your tokens: now you can never receive any
    // tokens again.
    Dead,
}

#[derive(Debug, Copy, Clone)]
pub struct RefInfo {
    state: RefState,
    // The reference this reference was derived from
    parent: Reference,
    // How many tokens this reference has
    num_tokens: u32,
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
    // Invariant: token_count should be equal to the sum of all values in
    // RefInfo.num_tokens.
    token_count: u32,
    // Invariant: if token_state is Exclusive, then the total number of tokens
    // in owners (sum of all values) should be 1.
    token_state: TokenState,
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
                state: RefState::Borrowing,
                num_tokens: 1,
                // Initial reference borrows from itself: this simplifies the code since
                // we don't have to consider two cases, one where a reference has a
                // parent and one where it doesn't.
                parent: initial_ref,
            },
        );

        (
            initial_ref,
            TokenMachine {
                ref_count: 1,
                token_state: TokenState::Exclusive,
                token_count: 1,
                ref_info,
            },
        )
    }

    // Initially tried to do reference without tracking the parent (instead
    // establishing the parent-child relationship upon first lending a token),
    // but that doesn't seem to justify the first optimization in the SB paper,
    // because it allows WRITE X, WRITE Y, READ X when X and Y are aliasing
    // pointers that derive from a common reference. (In that case, X can lend
    // to Y and return the token back to X for the read). This is made
    // impossible if you force X to return its token to the common ancestor
    // before being able to lend to Y.
    pub fn create_ref(&mut self, parent: Reference) -> Reference {
        let id = self.ref_count;
        self.ref_count += 1;
        let new_ref = Reference(id);

        self.ref_info.insert(
            new_ref,
            RefInfo {
                state: RefState::Created,
                parent,
                num_tokens: 0,
            },
        );

        new_ref
    }

    pub fn lend_token(&mut self, target: Reference) {
        let target_info = self.ref_info[&target];
        let source = target_info.parent;
        let source_info = self.ref_info[&source];

        // You must own a token to lend one out
        if source_info.num_tokens == 0 {
            panic!("Need to have a token to lend one out");
        }

        // Target must be ready to receive a token.
        match target_info.state {
            RefState::Created => {}
            RefState::Borrowing => {}
            RefState::Dead { .. } => panic!("Target cannot be dead"),
        };

        self.ref_info.get_mut(&source).unwrap().num_tokens -= 1;
        self.ref_info.get_mut(&target).unwrap().num_tokens += 1;

        self.ref_info.get_mut(&target).unwrap().state = RefState::Borrowing;
    }

    pub fn return_token(&mut self, source: Reference) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot give back a token if you don't have one");
        }

        let target = source_info.parent;

        self.ref_info.get_mut(&source).unwrap().num_tokens -= 1;
        self.ref_info.get_mut(&target).unwrap().num_tokens += 1;

        // If you've given back all your tokens, you become dead, and cannot
        // receive new tokens. However, you can still pass along tokens from
        // your children, even if you are dead. So it is possible that
        // num_tokens > 0 in this state.
        let source_info = self.ref_info.get_mut(&source).unwrap();
        if source_info.num_tokens == 0 {
            source_info.state = RefState::Dead;
        }
    }

    // TODO: Token duplication and state conversion (you can convert to
    // Exclusive if you hold all the tokens, and you can always go from
    // Exclusive to SharedRW or SharedRO).
    pub fn dup_token(&mut self, source: Reference) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot duplicate a token if you do not have a token");
        }

        if self.token_state == TokenState::Exclusive {
            panic!("Cannot duplicate exclusive token");
        }

        self.ref_info.get_mut(&source).unwrap().num_tokens += 1;
        self.token_count += 1;
    }

    pub fn merge_token(&mut self, source: Reference) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens <= 1 {
            panic!("Can only merge tokens if you have more than one");
        }

        self.ref_info.get_mut(&source).unwrap().num_tokens -= 1;
        self.token_count -= 1;
    }

    pub fn set_token_state(&mut self, token_state: TokenState) {
        // TODO: Maybe this could be less strict, where we allow some
        // transitions, like SharedRW to SharedRO even if the token is not
        // exclusively owned?
        if self.token_count != 1 {
            panic!("There must be exactly one token to change the token state");
        }

        self.token_state = token_state;
    }

    pub fn use_token(
        &mut self,
        source: Reference,
        access_kind: AccessKind,
        interior_mut: InteriorMut,
    ) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot perform accesses without a token");
        }

        if source_info.state == RefState::Dead {
            panic!("Cannot read/write with a dead reference");
        }

        match self.token_state {
            // You can do any kind of read/write if the token is exclusively
            // owned.
            TokenState::Exclusive => {}
            TokenState::SharedReadOnly => {
                if access_kind != AccessKind::Read {
                    panic!("Cannot only read with a SharedReadOnly token");
                }
            }
            TokenState::SharedReadWrite => {
                if interior_mut != InteriorMut::UnsafeCellOrRaw {
                    panic!("Can only do UnsafeCell/Raw access with a SharedReadWrite token");
                }
            }
        }
    }
}
