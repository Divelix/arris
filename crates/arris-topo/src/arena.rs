//! Chunked, shared slot storage: one arena per entity and geometry kind.
//!
//! The chunks sit behind `Arc`, so cloning an arena copies a list of
//! pointers and the first append after a clone copies only the tail chunk
//! (`docs/ARCHITECTURE.md` §The model). Every slot carries a generation;
//! a lookup resolves only when the id's generation is the slot's, so a
//! stale id never aliases a later occupant. A slot `retain` frees keeps
//! its place with its generation bumped and joins an ordered free set;
//! the next append fills the lowest free slot at that generation, so a
//! long-lived model does not grow without bound and ids stay
//! deterministic.

use std::collections::BTreeSet;
use std::sync::Arc;

/// Slots per chunk. Small enough that the tail-chunk copy after a clone is
/// a few kilobytes, large enough that a model of a hundred thousand
/// entities is a few hundred pointers per kind.
pub const CHUNK_SIZE: usize = 256;

#[derive(Debug, Clone)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

#[derive(Debug, Clone)]
struct Chunk<T> {
    slots: Vec<Slot<T>>,
}

impl<T> Chunk<T> {
    fn empty() -> Self {
        Chunk {
            slots: Vec::with_capacity(CHUNK_SIZE),
        }
    }
}

/// Slots of one kind, in creation order.
#[derive(Debug, Clone)]
pub(crate) struct Arena<T> {
    chunks: Vec<Arc<Chunk<T>>>,
    len: usize,
    /// Freed slots, lowest first: what the next append fills.
    free: BTreeSet<u32>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            chunks: Vec::new(),
            len: 0,
            free: BTreeSet::new(),
        }
    }
}

/// What a transaction records of an arena at entry: the length and the
/// free set, so a rollback can drop the tail and re-free the slots the
/// transaction filled.
#[derive(Debug, Clone)]
pub(crate) struct Mark {
    pub(crate) len: usize,
    free: BTreeSet<u32>,
}

impl<T: Clone> Arena<T> {
    /// The number of slots ever minted, freed ones included: the index the
    /// next append gets.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Stores `value` in the lowest freed slot at its bumped generation,
    /// or in a fresh slot at the end at generation zero, and returns
    /// `(index, generation)`. Only the chunk written is touched, and it
    /// is copied first if a clone shares it.
    pub(crate) fn push(&mut self, value: T) -> (u32, u32) {
        if let Some(&index) = self.free.first() {
            self.free.remove(&index);
            let slot = self.slot_mut(index as usize);
            slot.value = Some(value);
            return (index, slot.generation);
        }
        // The arena addresses slots by `u32`; running out of them is
        // resource exhaustion, the same class of failure as an allocation
        // that cannot be satisfied, and not a geometric condition.
        let index = u32::try_from(self.len).expect("the arena holds at most u32::MAX slots");
        if self.len % CHUNK_SIZE == 0 {
            self.chunks.push(Arc::new(Chunk::empty()));
        }
        let tail = self
            .chunks
            .last_mut()
            .expect("a chunk was just pushed or already exists");
        Arc::make_mut(tail).slots.push(Slot {
            generation: 0,
            value: Some(value),
        });
        self.len += 1;
        (index, 0)
    }

    /// The value at `index` when it is occupied and its generation is
    /// `generation`.
    pub(crate) fn get(&self, index: u32, generation: u32) -> Option<&T> {
        let index = index as usize;
        let slot = self
            .chunks
            .get(index / CHUNK_SIZE)?
            .slots
            .get(index % CHUNK_SIZE)?;
        (slot.generation == generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }

    /// The slot at `index`, which must exist; the chunk is copied first if
    /// a clone shares it.
    fn slot_mut(&mut self, index: usize) -> &mut Slot<T> {
        let chunk = self
            .chunks
            .get_mut(index / CHUNK_SIZE)
            .expect("a freed slot is inside the arena");
        &mut Arc::make_mut(chunk).slots[index % CHUNK_SIZE]
    }

    /// Frees the slot at `index` if it is live: the value is dropped, the
    /// generation bumped so every id minted for it stops resolving, and
    /// the slot joins the free set. Returns whether it was live.
    pub(crate) fn free_slot(&mut self, index: u32) -> bool {
        if self.value_at(index as usize).is_none() {
            return false;
        }
        let slot = self.slot_mut(index as usize);
        slot.value = None;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.insert(index);
        true
    }

    /// The transaction mark: what [`Arena::rollback`] restores.
    pub(crate) fn mark(&self) -> Mark {
        Mark {
            len: self.len,
            free: self.free.clone(),
        }
    }

    /// The slots that were free at `mark` and are filled now: what a
    /// rollback has to empty again besides the tail.
    pub(crate) fn reused_since(&self, mark: &Mark) -> Vec<u32> {
        mark.free
            .iter()
            .copied()
            .filter(|i| !self.free.contains(i))
            .collect()
    }

    /// Undoes every append since `mark`: the tail is dropped and the
    /// slots filled from the free set are emptied and freed again at the
    /// generation they had, so the ids the transaction minted are the
    /// ones the next appends get, as if the transaction had never run.
    /// Slots freed *during* the transaction stay freed: `retain` is not
    /// undone.
    pub(crate) fn rollback(&mut self, mark: &Mark) {
        for index in self.reused_since(mark) {
            let slot = self.slot_mut(index as usize);
            slot.value = None;
            self.free.insert(index);
        }
        self.truncate(mark.len);
    }

    /// The value at `index` whatever its generation, `None` for an empty
    /// or missing slot. For the arena's own bookkeeping (a rollback reads
    /// the slots it is about to drop); every id-based lookup goes through
    /// [`Arena::get`].
    pub(crate) fn value_at(&self, index: usize) -> Option<&T> {
        self.chunks
            .get(index / CHUNK_SIZE)?
            .slots
            .get(index % CHUNK_SIZE)?
            .value
            .as_ref()
    }

    /// Drops every slot from `len` on, so the next append gets index
    /// `len` again. A no-op when the arena is already that short; only the
    /// chunk that is cut is copied if a clone shares it.
    pub(crate) fn truncate(&mut self, len: usize) {
        if len >= self.len {
            return;
        }
        let keep_chunks = len.div_ceil(CHUNK_SIZE);
        self.chunks.truncate(keep_chunks);
        if let Some(tail) = self.chunks.last_mut() {
            let keep = len - (keep_chunks - 1) * CHUNK_SIZE;
            if tail.slots.len() > keep {
                Arc::make_mut(tail).slots.truncate(keep);
            }
        }
        self.len = len;
        self.free.retain(|&i| (i as usize) < len);
    }

    /// Appends a slot as it was stored — its generation and, for a live
    /// slot, its value — so a model read back from the native format has
    /// the same slots, freed ones included, and mints the same next id.
    pub(crate) fn push_slot(&mut self, generation: u32, value: Option<T>) -> u32 {
        let index = u32::try_from(self.len).expect("the arena holds at most u32::MAX slots");
        if self.len % CHUNK_SIZE == 0 {
            self.chunks.push(Arc::new(Chunk::empty()));
        }
        let tail = self
            .chunks
            .last_mut()
            .expect("a chunk was just pushed or already exists");
        let free = value.is_none();
        Arc::make_mut(tail).slots.push(Slot { generation, value });
        self.len += 1;
        if free {
            self.free.insert(index);
        }
        index
    }

    /// The number of live slots.
    #[cfg(test)]
    pub(crate) fn live(&self) -> usize {
        self.len - self.free.len()
    }

    /// Every slot in index order as `(generation, value)`, freed slots
    /// with `None`.
    pub(crate) fn slots(&self) -> impl Iterator<Item = (u32, Option<&T>)> {
        self.chunks
            .iter()
            .flat_map(|c| c.slots.iter().map(|s| (s.generation, s.value.as_ref())))
    }

    /// `true` when chunk `i` of both arenas is the same allocation.
    #[cfg(test)]
    pub(crate) fn shares_chunk(&self, other: &Self, i: usize) -> bool {
        match (self.chunks.get(i), other.chunks.get(i)) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// The number of chunks.
    #[cfg(test)]
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_are_sequential_and_chunks_fill_in_order() {
        let mut a = Arena::default();
        for i in 0..(2 * CHUNK_SIZE + 1) {
            assert_eq!(a.push(i), (i as u32, 0));
        }
        assert_eq!(a.chunk_count(), 3);
        assert_eq!(a.len(), 2 * CHUNK_SIZE + 1);
        assert_eq!(a.get(0, 0), Some(&0));
        assert_eq!(a.get(CHUNK_SIZE as u32, 0), Some(&CHUNK_SIZE));
        assert_eq!(a.get(2 * CHUNK_SIZE as u32 + 1, 0), None, "past the end");
        assert_eq!(a.get(3, 1), None, "a stale generation never resolves");
    }

    #[test]
    fn a_clone_shares_every_chunk_and_an_append_copies_only_the_tail() {
        let mut a = Arena::default();
        for i in 0..(CHUNK_SIZE + 10) {
            a.push(i);
        }
        let mut b = a.clone();
        assert!(a.shares_chunk(&b, 0) && a.shares_chunk(&b, 1));
        b.push(usize::MAX);
        assert!(a.shares_chunk(&b, 0), "the full chunk is still shared");
        assert!(
            !a.shares_chunk(&b, 1),
            "the tail was copied before the append"
        );
        assert_eq!(a.get(CHUNK_SIZE as u32 + 10, 0), None);
        assert_eq!(b.get(CHUNK_SIZE as u32 + 10, 0), Some(&usize::MAX));
    }

    #[test]
    fn a_freed_slot_is_refilled_lowest_first_at_the_bumped_generation() {
        let mut a = Arena::default();
        for i in 0..5 {
            a.push(i);
        }
        assert!(a.free_slot(3) && a.free_slot(1));
        assert!(!a.free_slot(1), "already free");
        assert_eq!(a.get(1, 0), None, "the old id no longer resolves");
        assert_eq!(a.live(), 3);
        assert_eq!(a.push(10), (1, 1), "the lowest free slot, generation one");
        assert_eq!(a.get(1, 1), Some(&10));
        assert_eq!(a.get(1, 0), None, "and the stale id still does not");
        assert_eq!(a.push(11), (3, 1));
        assert_eq!(a.push(12), (5, 0), "then the tail");
        a.free_slot(1);
        assert_eq!(a.push(13), (1, 2), "every free bumps the generation");
    }

    #[test]
    fn rollback_empties_the_reused_slots_and_drops_the_tail() {
        let mut a = Arena::default();
        for i in 0..4 {
            a.push(i);
        }
        a.free_slot(2);
        let mark = a.mark();
        assert_eq!(a.push(20), (2, 1));
        assert_eq!(a.push(21), (4, 0));
        assert_eq!(a.reused_since(&mark), [2]);
        a.rollback(&mark);
        assert_eq!(a.len(), 4);
        assert_eq!(a.get(2, 1), None);
        assert_eq!(
            a.push(22),
            (2, 1),
            "the same id as the rolled-back append got"
        );
        // A slot freed inside the transaction stays freed.
        let mark = a.mark();
        a.free_slot(0);
        a.rollback(&mark);
        assert_eq!(a.get(0, 0), None);
        assert_eq!(a.push(23), (0, 1));
    }

    #[test]
    fn truncate_drops_whole_chunks_and_part_of_the_tail() {
        let mut a = Arena::default();
        for i in 0..(2 * CHUNK_SIZE + 5) {
            a.push(i);
        }
        let shared = a.clone();
        a.truncate(CHUNK_SIZE + 3);
        assert_eq!(a.len(), CHUNK_SIZE + 3);
        assert_eq!(a.chunk_count(), 2);
        assert_eq!(a.get(CHUNK_SIZE as u32 + 2, 0), Some(&(CHUNK_SIZE + 2)));
        assert_eq!(a.get(CHUNK_SIZE as u32 + 3, 0), None);
        assert!(a.shares_chunk(&shared, 0), "untouched chunks stay shared");
        assert!(!a.shares_chunk(&shared, 1), "the cut chunk was copied");
        assert_eq!(
            shared.len(),
            2 * CHUNK_SIZE + 5,
            "the clone kept everything"
        );
        assert_eq!(
            a.push(7),
            (CHUNK_SIZE as u32 + 3, 0),
            "the next index is the new length"
        );
        a.truncate(0);
        assert_eq!((a.len(), a.chunk_count()), (0, 0));
        a.truncate(10);
        assert_eq!(a.len(), 0, "truncating past the end is a no-op");
    }
}
