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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TokenExclusivity {
    Shared,
    Exclusive,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TokenPermissions {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TokenInfo(TokenExclusivity, TokenPermissions);

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
                // TODO: Allow delivering token more than once: this allows a
                // shared token to be upgraded to an exclusive token by sending
                // more token pieces from below.

                // Need to increment num_splits when you do so, in order to make
                // sure that all such tokens get sent back.
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

    pub fn set_token_perms(&mut self, source: Reference, token_perms: TokenPermissions) {
        // Changing the state of the token requires exclusive ownership of it.
        let token_info = self
            .get_token_info(source)
            .expect("have to own token to change its state");

        if token_info.0 != TokenExclusivity::Exclusive {
            panic!("Need to have exclusive ownership of the token to change its state");
        }

        self.token_perms = token_perms;
    }

    fn get_token_info(&self, source: Reference) -> Option<TokenInfo> {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            return None;
        }

        // You should not have tokens if you're dead, because being dead means
        // you gave your token back entirely.
        assert!(source_info.state != RefState::Dead);

        let exclusivity = if self.token_count == 1 {
            TokenExclusivity::Exclusive
        } else {
            TokenExclusivity::Shared
        };

        let perms = self.token_perms;

        Some(TokenInfo(exclusivity, perms))
    }

    // Not keeping track of the type of reference doesn't work for the second
    // optimization in the SB paper. This is because that optimization would not
    // be allowed for a mutable reference.
    pub fn use_token(&mut self, source: Reference, access_kind: AccessKind) {
        let token_info = self
            .get_token_info(source)
            .expect("Cannot read/write without a token");

        match self.ref_info[&source].kind {
            RefKind::SharedReadOnly => {
                match access_kind {
                    AccessKind::Read => {
                        // Reading can be done if there are no writers, so you either need a shared read-only token or an exclusive token.
                        if !(token_info
                            == TokenInfo(TokenExclusivity::Shared, TokenPermissions::ReadOnly)
                            || token_info.0 == TokenExclusivity::Exclusive)
                        {
                            panic!(
                                "Cannot read with shared read-only reference if there are writers"
                            );
                        }
                    }
                    AccessKind::Write => panic!("Cannot write with read-only reference"),
                }
            }
            RefKind::SharedReadWrite => {
                match access_kind {
                    // Can read with any kind of token, shared/exclusive and
                    // read-only or read-write.
                    AccessKind::Read => {}
                    AccessKind::Write => {
                        // Writing requires (shared/exclusive) read-write token
                        if !(token_info.1 == TokenPermissions::ReadWrite) {
                            panic!("Writing using SharedRW requires read-write token");
                        }
                    }
                }
            }
            RefKind::Unique => {
                match access_kind {
                    AccessKind::Read => {
                        // Reading can be done if there are no writers, so you either need a shared read-only token or an exclusive token.
                        if !(token_info
                            == TokenInfo(TokenExclusivity::Shared, TokenPermissions::ReadOnly)
                            || token_info.0 == TokenExclusivity::Exclusive)
                        {
                            panic!("Cannot read with unique reference if there are writers");
                        }
                    }
                    AccessKind::Write => {
                        // Writing requires exclusive read-write access.
                        if !(token_info
                            == TokenInfo(TokenExclusivity::Exclusive, TokenPermissions::ReadWrite))
                        {
                            panic!("Writing with unique reference requires exclusive read-write access");
                        }
                    }
                }
            }
        }
    }
}
