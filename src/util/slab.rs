use std::collections::LinkedList;

/// A data structure with preallocated space for elements.
/// The slab will find empty slots for new elements and return the index where it was inserted.
pub struct Slab<T> {
    data: Vec<Option<T>>,
    open_slots: LinkedList<usize>,
}

impl<T> Slab<T> {
    /// Creates an empty slots structure with no allocation.
    /// Allocation will occur on the first insertion.
    pub fn new() -> Slab<T> {
        Self::with_capacity(0)
    }

    /// Creates a new slab with the given capacity. "capacity" specifies how many empty slots will be allocated.
    pub fn with_capacity(capacity: usize) -> Slab<T> {
        Slab { data: Vec::with_capacity(capacity), open_slots: LinkedList::new() }
    }

    /// Gets the key of the next element that will be inserted.
    pub fn next_key(&self) -> usize {
        self.open_slots.front().cloned().unwrap_or(self.data.len())
    }

    /// Finds an empty slot and populates it with the given element. Returns the key of the slot, starting at 0.
    /// If there are no empty slots, then the capacity of the slab is doubled and new memory is allocated.
    pub fn insert(&mut self, inner: T) -> usize {
        if let Some(index) = self.open_slots.pop_front() {
            self.data.get_mut(index).unwrap().replace(inner);
            index
        } else {
            self.data.push(Some(inner));
            self.data.len() - 1
        }
    }

    /// Gets a reference to the element with the given key.
    pub fn get(&self, key: usize) -> Option<&T> {
        self.data.get(key).and_then(|e| e.as_ref())
    }

    /// Removes the element at the given key, allowing another element to be inserted for the key.
    pub fn remove(&mut self, key: usize) -> Option<T> {
        if let Some(slot) = self.data.get_mut(key) {
            if slot.is_some() {
                self.open_slots.push_front(key);
                return slot.take();
            }
        }
        return None;
    }
}

#[cfg(test)]
mod tests {
    use crate::util::slab::Slab;

    #[test]
    fn test_insertion() {
        let mut slab = Slab::with_capacity(2);

        let x = slab.next_key();
        assert_eq!(x, slab.insert("a"));
        let y = slab.next_key();
        assert_eq!(y, slab.insert("b"));
        let z = slab.next_key();
        assert_eq!(z, slab.insert("c"));
        let w = slab.next_key();
        assert_eq!(w, slab.insert("d"));

        assert_eq!(&"a", slab.get(x).unwrap());
        assert_eq!(&"b", slab.get(y).unwrap());
        assert_eq!(&"c", slab.get(z).unwrap());
        assert_eq!(&"d", slab.get(w).unwrap());
    }

    #[test]
    fn test_removal() {
        let mut slab = Slab::with_capacity(2);

        let x =  slab.insert("a");
        let y =  slab.insert("b");
        let z =  slab.insert("c");
        let w =  slab.insert("d");

        slab.remove(x);
        slab.remove(y);
        slab.remove(z);
        slab.remove(w);

        assert!(slab.get(x).is_none());
        assert!(slab.get(y).is_none());
        assert!(slab.get(z).is_none());
        assert!(slab.get(w).is_none());
    }

    #[test]
    fn double_removal(){
        let mut slab = Slab::new();

        let x = slab.insert("a");

        assert!(slab.remove(x).is_some());
        assert!(slab.remove(x).is_none());

        let x = slab.insert("a");

        assert!(slab.get(x).is_some())
    }

    #[test]
    fn key_out_of_bounds() {
        let slab = Slab::<String>::new();

        assert!(slab.get(1000).is_none())
    }

    #[test]
    fn remove_key_out_of_bounds() {
        let mut slab = Slab::<String>::new();

        assert!(slab.remove(1000).is_none())
    }
}
