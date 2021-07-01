use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefState {
    // This state means you've never held a tokens.
    Created,
    // This state means you've received a token at some point, but you may have
    // passed it on to someone else.
    Borrowing,
    // This state means you've given back the token you've received. (In its
    // entirety, and not only some split piece of it).
    Dead,
}

// TODO: Is it necessary to have three kinds? What about immutable/mutable and a
// flag on the accesses indicating interior mutability? That would allow you to
// "cast away" interior mutability before using the reference, though. Probably
// safest to require changing the reference kind to involve a retagging.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefKind {
    SharedReadOnly,
    SharedReadWrite,
    Unique,
}

#[derive(Debug, Copy, Clone)]
pub struct RefInfo {
    kind: RefKind,
    state: RefState,
    // The reference this reference was derived from
    parent: Reference,
    // How many token pieces this reference has
    num_tokens: u32,
    // Into how many pieces has this reference fragmented its part of a token?
    // This is used to ensure that a reference must give back the entire token
    // it has received, and not just some smaller portion of it.
    num_splits: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TokenPermissions {
    ReadOnly,
    ReadWrite,
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
    ref_info: HashMap<Reference, RefInfo>,
    token_perms: TokenPermissions,
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
                kind: RefKind::Unique,
                state: RefState::Borrowing,
                num_tokens: 1,
                num_splits: 0,
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
                token_count: 1,
                ref_info,
                token_perms: TokenPermissions::ReadWrite,
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
    pub fn create_ref(&mut self, parent: Reference, kind: RefKind) -> Reference {
        let parent_info = self.ref_info[&parent];
        if parent_info.kind == RefKind::SharedReadOnly && kind != RefKind::SharedReadOnly {
            // Prevent read-only reference from spawning mutable references and
            // using them to mutate.
            panic!("Cannot create mutable reference from immutable reference");
        }

        let id = self.ref_count;
        self.ref_count += 1;
        let new_ref = Reference(id);

        self.ref_info.insert(
            new_ref,
            RefInfo {
                kind,
                state: RefState::Created,
                parent,
                num_tokens: 0,
                num_splits: 0,
            },
        );

        new_ref
    }

    pub fn borrow_token(&mut self, target: Reference) {
        let target_info = self.ref_info[&target];
        let source = target_info.parent;
        let source_info = self.ref_info[&source];

        // Source must own a token to lend one out
        if source_info.num_tokens == 0 {
            panic!("Need to have a token to lend one out");
        }

        // Target must be ready to receive a token.
        match target_info.state {
            RefState::Created => {}
            RefState::Borrowing => {
                panic!("Target has already received a token before")
            }
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

        if source_info.num_splits > 0 {
            panic!("Can only give back the entire token and not just some piece of it");
        }

        assert!(source_info.num_tokens == 1);

        let target = source_info.parent;

        self.ref_info.get_mut(&source).unwrap().num_tokens -= 1;
        self.ref_info.get_mut(&target).unwrap().num_tokens += 1;

        self.ref_info.get_mut(&source).unwrap().state = RefState::Dead;
    }

    pub fn dup_token(&mut self, source: Reference) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot duplicate a token if you do not have a token");
        }

        let source_info = self.ref_info.get_mut(&source).unwrap();
        source_info.num_tokens += 1;
        source_info.num_splits += 1;
        self.token_count += 1;
    }

    pub fn merge_token(&mut self, source: Reference) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens <= 1 {
            panic!("Can only merge tokens if you have more than one");
        }

        let source_info = self.ref_info.get_mut(&source).unwrap();
        source_info.num_tokens -= 1;
        source_info.num_splits -= 1;
        self.token_count -= 1;
    }

    pub fn set_read_only_state(&mut self, source: Reference, token_perms: TokenPermissions) {
        // Changing the state of the token counts as a write.
        self.use_token(source, AccessKind::Write);

        self.token_perms = token_perms;
    }

    // Not keeping track of the type of reference doesn't work for the second
    // optimization in the SB paper. This is because that optimization would not
    // be allowed for a mutable reference.
    pub fn use_token(&mut self, source: Reference, access_kind: AccessKind) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot perform accesses without a token");
        }

        // You should not have tokens if you're dead, because being dead means
        // you gave your token back entirely.
        assert!(source_info.state != RefState::Dead);

        match source_info.kind {
            // Note: reading should not be allowed even if you have exclusive
            // ownership of a read-write token, because a read-write token could
            // be passed along to a child and used for writing before returning
            // it to the read-only reference.
            RefKind::SharedReadOnly => {
                match access_kind {
                    AccessKind::Write => panic!("Cannot write with read-only reference"),
                    AccessKind::Read => {
                        // Reading requires a read-only token or exclusive read-write token.
                        if !(self.token_perms == TokenPermissions::ReadOnly
                            || (self.token_count == 1
                                && self.token_perms == TokenPermissions::ReadWrite))
                        {
                            panic!("Reading with SharedRO reference requires read-only token or exclusive read-write token");
                        }
                    }
                }
            }
            RefKind::SharedReadWrite => match access_kind {
                AccessKind::Read => {
                    // Reading can be done using both kinds of tokens
                }
                AccessKind::Write => {
                    // Writing requires read-write token (does not have to be exclusive)
                    if self.token_perms != TokenPermissions::ReadWrite {
                        panic!("Writing requires read-write token");
                    }
                }
            },
            RefKind::Unique => {
                // If the reference is alone, it can do anything, INCLUDING
                // WRITING WITH A READ-ONLY TOKEN. This allows such references
                // to change the state of the token from read-only to
                // read-write.
                if self.token_count == 1 {
                    return;
                }

                // If it's not alone, it behaves like a SharedRO reference.
                match access_kind {
                    AccessKind::Write => panic!("Can only write if completely alone"),
                    AccessKind::Read => {
                        // Reading requires a read-only token (not a read-write token)
                        if self.token_perms != TokenPermissions::ReadOnly {
                            panic!("Reading with Unique cannot be done if others can write");
                        }
                    }
                }
            }
        }
    }
}
