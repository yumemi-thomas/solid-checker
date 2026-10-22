//! Symbol interning (Go: `symbol_intern.go`). A symbol id is a ~33-byte
//! string, and per-generation bookkeeping keyed by it paid string hashing and
//! map growth proportional to the whole symbol universe. The interner assigns
//! each id a dense session-stable handle once; the per-generation sets become
//! bitsets over handles.

use crate::model::SymbolId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Default)]
pub struct SymbolInterner {
    handles: HashMap<SymbolId, i32>,
    count: i32,
}

/// An interner shared by the sets built over it.
pub type SharedInterner = Rc<RefCell<SymbolInterner>>;

impl SymbolInterner {
    #[must_use]
    pub fn new_shared() -> SharedInterner {
        Rc::new(RefCell::new(Self::default()))
    }

    /// Interns `id`, assigning the next dense handle on first sight.
    pub fn handle(&mut self, id: &SymbolId) -> i32 {
        if let Some(handle) = self.handles.get(id) {
            return *handle;
        }
        let handle = self.count;
        self.handles.insert(id.clone(), handle);
        self.count += 1;
        handle
    }

    /// Reports `id`'s handle without interning it.
    #[must_use]
    pub fn lookup(&self, id: &str) -> Option<i32> {
        self.handles.get(id).copied()
    }

    #[must_use]
    pub fn size(&self) -> usize {
        usize::try_from(self.count).unwrap_or(0)
    }
}

/// One generation's symbol membership as a bitset over interned handles. `add`
/// grows the backing as the interner grows mid-build; `members.len()` is the
/// addressable handle range.
#[derive(Debug)]
pub struct SymbolHandleSet {
    interner: SharedInterner,
    pub members: Vec<bool>,
    count: usize,
}

impl SymbolHandleSet {
    /// An empty set on top of `scratch`, grown to the interner's current size.
    #[must_use]
    pub fn new(interner: &SharedInterner, scratch: Vec<bool>) -> Self {
        let size = interner.borrow().size();
        let mut members = scratch;
        if members.capacity() < size {
            members = vec![false; size];
        } else {
            let capacity = members.capacity();
            members.clear();
            members.resize(capacity, false);
        }
        Self {
            interner: Rc::clone(interner),
            members,
            count: 0,
        }
    }

    /// Reports whether `handle` was newly inserted.
    pub fn add(&mut self, handle: i32) -> bool {
        let Ok(slot) = usize::try_from(handle) else {
            return false;
        };
        if slot >= self.members.len() {
            // Cold materialization interns symbols while it builds the set.
            // Growing exactly to the interner's size here would copy the
            // complete bitset once per newly discovered symbol: quadratic
            // copying for a set whose final representation is already dense.
            // Grow geometrically instead.
            let size = self.interner.borrow().size();
            let next = 64.max(self.members.len() * 2).max(size).max(slot + 1);
            self.members.resize(next, false);
        }
        if self.members[slot] {
            return false;
        }
        self.members[slot] = true;
        self.count += 1;
        true
    }

    /// Replaces the membership wholesale and recounts it (Go: assigning
    /// `members` from `copyHandleMembership` and recounting).
    pub fn replace_members(&mut self, source: &[bool]) {
        self.members = copy_handle_membership(source, std::mem::take(&mut self.members));
        self.count = self.members.iter().filter(|member| **member).count();
    }

    pub fn add_id(&mut self, id: &SymbolId) -> bool {
        let handle = self.interner.borrow_mut().handle(id);
        self.add(handle)
    }

    #[must_use]
    pub fn contains(&self, handle: i32) -> bool {
        usize::try_from(handle)
            .ok()
            .is_some_and(|slot| self.members.get(slot).copied().unwrap_or(false))
    }

    /// Membership without interning unknown ids.
    #[must_use]
    pub fn contains_id(&self, id: &str) -> bool {
        self.interner
            .borrow()
            .lookup(id)
            .is_some_and(|handle| self.contains(handle))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// A [`SymbolHandleSet`] that also keeps the inserted ids as a list, for the
/// consumers that iterate the changed set (the transport manifest) or size by
/// it.
#[derive(Debug)]
pub struct ChangedSymbolSet {
    set: SymbolHandleSet,
    pub ids: Vec<SymbolId>,
}

impl ChangedSymbolSet {
    #[must_use]
    pub fn new(interner: &SharedInterner) -> Self {
        Self {
            set: SymbolHandleSet::new(interner, Vec::new()),
            ids: Vec::new(),
        }
    }

    pub fn add(&mut self, id: &SymbolId) {
        if self.set.add_id(id) {
            self.ids.push(id.clone());
        }
    }

    #[must_use]
    pub fn contains_id(&self, id: &str) -> bool {
        self.set.contains_id(id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Dead identities may outnumber the live universe without bound when a
/// session keeps minting non-durable ids; past this slack the cheapest
/// eviction is a fresh interner (Go: `internerResetSlack`).
pub const INTERNER_RESET_SLACK: usize = 1024;

/// Copies `source` into `scratch`, reusing its allocation when it fits (Go:
/// `copyHandleMembership`).
#[must_use]
pub fn copy_handle_membership(source: &[bool], mut scratch: Vec<bool>) -> Vec<bool> {
    scratch.clear();
    scratch.extend_from_slice(source);
    scratch
}

/// Whether two handle memberships agree, treating the shorter as padded with
/// absent (Go: `equalHandleMembership`).
#[must_use]
pub fn equal_handle_membership(left: &[bool], right: &[bool]) -> bool {
    let common = left.len().min(right.len());
    left[..common] == right[..common]
        && left[common..].iter().all(|member| !member)
        && right[common..].iter().all(|member| !member)
}
