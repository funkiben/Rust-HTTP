use std::cmp::min;
use std::collections::LinkedList;
use std::marker::PhantomData;

/// A data structure that preallocates slots that elements are inserted into.
/// Slots may contain reusable system resources that are used by inserted elements.
pub struct Slots<I, F, E> {
    data: Vec<Slot<I, F, E>>,
    open_slots: LinkedList<usize>,
}

impl<I, F: ToEmptySlot<I, E>, E: Default + ToFilledSlot<I, F>> Slots<I, F, E> {
    /// Creates an empty slots structure with no allocation.
    /// Allocation will occur on the first insertion.
    pub fn new() -> Slots<I, F, E> {
        Slots { data: vec![], open_slots: LinkedList::new() }
    }

    /// Creates a new slots structure with the given capacity. "capacity" specifies how many empty slots will be allocated.
    pub fn with_capacity(capacity: usize) -> Slots<I, F, E> {
        let mut slots = Self::new();
        slots.add_capacity(capacity);
        slots
    }

    /// Finds an empty slot and populates it with the given element. Returns the key of the slot.
    /// If there are no empty slots, then the capacity of the structure is doubled and new empty slots are allocated.
    pub fn insert(&mut self, inner: I) -> usize {
        if let Some(index) = self.open_slots.pop_front() {
            self.data.get_mut(index).unwrap().fill(inner);
            index
        } else {
            self.add_capacity(min(self.data.capacity(), 2));
            self.insert(inner)
        }
    }

    /// Gets a occupied slot with the given key. Returns None if the key is invalid or the slot is not occupied.
    pub fn get(&self, key: usize) -> Option<&F> {
        self.data.get(key).and_then(|slot| slot.filled_ref())
    }

    pub fn get_mut(&mut self, key: usize) -> Option<&mut F> {
        self.data.get_mut(key).and_then(|slot| slot.filled_mut())
    }

    /// Removes the element at the given key, freeing the slot up to be re-used.
    pub fn remove(&mut self, key: usize) -> Option<I> {
        if let Some(slot) = self.data.get_mut(key) {
            self.open_slots.push_front(key);
            slot.empty()
        } else {
            None
        }
    }

    /// Allocates new empty slots.
    fn add_capacity(&mut self, size: usize) {
        self.data.reserve(size);
        for i in 1..=size {
            self.data.push(Slot::default());
            self.open_slots.push_front(self.data.capacity() - i);
        }
    }
}

enum Slot<I, F, E> {
    Filled(Option<F>),
    Empty(Option<E>),
    _Phantom(PhantomData<I>),
}

impl<I, F: ToEmptySlot<I, E>, E: ToFilledSlot<I, F>> Slot<I, F, E> {
    fn fill(&mut self, inner: I) {
        match self {
            Slot::Empty(empty) => {
                let fill = empty.take().unwrap().into_filled_slot(inner);
                *self = Slot::Filled(Some(fill))
            }
            _ => panic!("Filled an already filled slot!")
        }
    }

    fn empty(&mut self) -> Option<I> {
        match self {
            Slot::Filled(fill) => {
                let (inner, empty) = fill.take().unwrap().into_empty_slot();
                *self = Slot::Empty(Some(empty));
                Some(inner)
            }
            _ => None
        }
    }

    fn filled_ref(&self) -> Option<&F> {
        match self {
            Slot::Filled(fill) => fill.as_ref(),
            _ => None
        }
    }

    fn filled_mut(&mut self) -> Option<&mut F> {
        match self {
            Slot::Filled(fill) => fill.as_mut(),
            _ => None
        }
    }
}

impl<I, F, E: Default> Default for Slot<I, F, E> {
    fn default() -> Self {
        Slot::Empty(Some(E::default()))
    }
}

pub trait ToFilledSlot<Inner, FilledSlot> {
    fn into_filled_slot(self, inner: Inner) -> FilledSlot;
}

pub trait ToEmptySlot<Inner, EmptySlot> {
    fn into_empty_slot(self) -> (Inner, EmptySlot);
}