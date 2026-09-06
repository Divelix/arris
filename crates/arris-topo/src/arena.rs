//! Chunked, shared slot storage: one arena per entity and geometry kind.
//!
//! The chunks sit behind `Arc`, so cloning an arena copies a list of
//! pointers and the first append after a clone copies only the tail chunk
//! (`docs/01-architecture.md` §The model). Every slot carries a generation;
//! a lookup resolves only when the id's generation is the slot's, so a
//! stale id never aliases a later occupant.

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
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            chunks: Vec::new(),
            len: 0,
        }
    }
}

impl<T: Clone> Arena<T> {
    /// Appends `value` in a fresh slot and returns `(index, generation)`.
    /// Only the tail chunk is touched, and it is copied first if a clone
    /// shares it.
    pub(crate) fn push(&mut self, value: T) -> (u32, u32) {
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
}
