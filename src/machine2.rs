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
    kind: RefKind,
    state: RefState,
    // The reference this reference was derived from
    parent: Reference,
    // How many tokens this reference has
    num_tokens: u32,
}

// TODO: Update comments
// TODO: Is it necessary to have three kinds? What about immutable/mutable and a
// flag on the accesses indicating interior mutability? That would allow you to
// "cast away" interior mutability before using the reference, though. Probably
// safest to require changing the reference kind to involve a retagging.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefKind {
    SharedReadOnly,
    SharedReadWrite,
    ExclusiveReadWrite,
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
                kind: RefKind::ExclusiveReadWrite,
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

    pub fn dup_token(&mut self, source: Reference) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot duplicate a token if you do not have a token");
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

    pub fn set_read_only_state(&mut self, source: Reference, token_perms: TokenPermissions) {
        // Changing the state of the token counts as a write.
        self.use_token(source, AccessKind::Write);

        self.token_perms = token_perms;
    }

    // None means there is no frame: there is only one token.
    fn frame_permissions(&mut self) -> Option<TokenPermissions> {
        if self.token_count == 1 {
            return None;
        }

        return Some(self.token_perms);
    }

    // This is a partial order
    fn permissions_bounded(
        frame: Option<TokenPermissions>,
        maximum: Option<TokenPermissions>,
    ) -> bool {
        match (frame, maximum) {
            (None, _) => true,
            (_, None) => false,
            (Some(TokenPermissions::ReadOnly), Some(TokenPermissions::ReadOnly)) => true,
            (Some(TokenPermissions::ReadOnly), Some(TokenPermissions::ReadWrite)) => true,
            (Some(TokenPermissions::ReadWrite), Some(TokenPermissions::ReadOnly)) => false,
            (Some(TokenPermissions::ReadWrite), Some(TokenPermissions::ReadWrite)) => true,
        }
    }

    // Not keeping track of the type of reference doesn't work for the second
    // optimization in the SB paper. This is because that optimization would not
    // be allowed for a mutable reference.
    pub fn use_token(&mut self, source: Reference, access_kind: AccessKind) {
        let source_info = self.ref_info[&source];

        if source_info.num_tokens == 0 {
            panic!("Cannot perform accesses without a token");
        }

        if source_info.state == RefState::Dead {
            panic!("Cannot read/write with a dead reference");
        }

        match source_info.kind {
            RefKind::SharedReadOnly => {
                match access_kind {
                    AccessKind::Write => panic!("Cannot write with read-only reference"),
                    AccessKind::Read => {
                        if !TokenMachine::permissions_bounded(
                            self.frame_permissions(),
                            Some(TokenPermissions::ReadOnly),
                        ) {
                            panic!("Cannot read using read-only reference if frame has write permission");
                        }
                    }
                }
            }
            RefKind::SharedReadWrite => match access_kind {
                AccessKind::Read => {}
                AccessKind::Write => {
                    if self.token_perms == TokenPermissions::ReadOnly {
                        panic!("Cannot write with read-only token");
                    }
                }
            },
            RefKind::ExclusiveReadWrite => match access_kind {
                AccessKind::Read => {
                    if !TokenMachine::permissions_bounded(
                        self.frame_permissions(),
                        Some(TokenPermissions::ReadOnly),
                    ) {
                        panic!("Cannot read with unique reference if frame has write permissions");
                    }
                }
                AccessKind::Write => {
                    if self.token_perms == TokenPermissions::ReadOnly {
                        panic!("Cannot write with read-only token");
                    }

                    if !TokenMachine::permissions_bounded(self.frame_permissions(), None) {
                        panic!("Cannot write with unique reference if frame has any permissions");
                    }
                }
            },
        }
    }
}
