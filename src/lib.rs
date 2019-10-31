/*
requirements:
    - owned storage
    - iteration over the items
    - immediate deltion of dropped items
optional:
    - fast iteration
    - continuous memory

observations: 
    - the set owns the values, so they cannot be inserted in another set
    - the references need to reference the set in order to remove the items on drop
    - the set itself needs to be reference counted in case it is dropped while references are alive
    - it is a set, so the storage can be a Vec and the indices used to reference entries.
    - we need interior mutabiltiy as the set is shared

decisions: 
    - deletion sets entries to "empty" (avoids double references)
    - store values directly. the user can use WeakSet<Box<T>> to change this
*/

use std::rc::Rc;
use std::cell::RefCell;

pub struct WeakSet<T> {
    inner: Rc<RefCell<WeakSetInner<T>>>
}

// #[derive(Clone)] fails, so do it manually
impl<T> Clone for WeakSet<T> {
    fn clone(&self) -> Self {
        WeakSet {
            inner: self.inner.clone()
        }
    }
}

pub struct WeakSetEntry<T> {
    set: WeakSet<T>,
    index: usize
}

struct WeakSetInner<T> {
    slots: Vec<WeakSetSlot<T>>
}

// this isn't `Option<(T, usize)>` because we might want to add information to `Empty`.
// - previous and next used entry index for fast iteration
// - the next free entry index for fast insertion
enum WeakSetSlot<T> {
    // deleted 
    Empty,

    // used with the number of references
    Used(T, usize)
}

impl<T> WeakSet<T> {
    pub fn new() -> WeakSet<T> {
        WeakSet {
            inner: Rc::new(RefCell::new(WeakSetInner { slots: Vec::new() })) 
        }
    }

    // note: this needs &mut self to ensure proper iterator behaviour.
    // see iter() for details.
    pub fn insert(&mut self, val: T) -> WeakSetEntry<T> {
        // get a mutable reference
        let mut inner = self.inner.borrow_mut();
        
        // try to find a 'Free' slot first, otherwise add one
        let slot_idx = inner.slots.iter().position(|slot|
            match slot {
                WeakSetSlot::Empty => true,
                _ => false
            }
        ).unwrap_or_else(|| {
            inner.slots.push(WeakSetSlot::Empty);
            inner.slots.len() - 1
        });

        // construct an entry with one reference
        let new_slot = WeakSetSlot::Used(val, 1);

        // and assign it to the index (we could check that the previous value was `Empty`…)
        inner.slots[slot_idx] = new_slot;

        // finally construct a reference to it
        WeakSetEntry {
            set: self.clone(),
            index: slot_idx
        }
    }

    // common method to create an entry from thin air
    fn make_entry(&self, index: usize) -> Option<WeakSetEntry<T>> {
        match self.inner.borrow_mut().slots[index] {
            WeakSetSlot::Empty => None,
            WeakSetSlot::Used(_, ref mut refcount) => {
                // we are creating a new referernce, so bump the refcount
                *refcount += 1;
                Some(WeakSetEntry {
                    set: self.clone(),
                    index
                })
            }
        }
    }

    // decrease the refcount of the given entry, possibly dropping it
    fn drop_entry(&self, index: usize) {
        // get a reference to the slot
        let ref mut slot = self.inner.borrow_mut().slots[index];
        let is_empty = match slot {
            &mut WeakSetSlot::Used(_, ref mut refcount) => {
                // decrement the refcount and see if it is zero
                *refcount -= 1;
                *refcount == 0
            },
            _ => unreachable!()
        };

        // if it is empty now, set the slot to empty (dropping the value in the process)
        if is_empty {
            *slot = WeakSetSlot::Empty;
        }
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item=WeakSetEntry<T>> + 'a {
        // This is actually not easy.
        // Items can be dropped any time during iteration.
        // The good news is that at least no new items can be inserted (hence insert takes &mut self),
        // meaning we can use indices for iteration.
        // We cannot borrow the inner storage for the iterator lifetime.

        // the highest possible slot
        let max_idx = self.inner.borrow().slots.len();
        (0 .. max_idx).filter_map(move |idx| self.make_entry(idx))
    }
}

impl<T> Drop for WeakSetEntry<T> {
    fn drop(&mut self) {
        self.set.drop_entry(self.index);
    }
}

impl<T> Clone for WeakSetEntry<T> {
    fn clone(&self) -> Self {
        self.set.make_entry(self.index).unwrap()
    }
}

use std::fmt;
/*
old debug impl
problem: 
    the debug print creates an iterator which creates additional references to each item
    so we get higher numbers…

impl<T: fmt::Debug> fmt::Debug for WeakSetEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.set.inner.borrow().slots[self.index] {
            WeakSetSlot::Empty => write!(f, "free"),
            WeakSetSlot::Used(ref val, refcount) => write!(f, "{:?}({})", val, refcount)
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for WeakSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
*/

impl<T: fmt::Debug> fmt::Debug for WeakSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // this debug impl does not create references. 
        // we can do this because we control the iterator and can be sure nothing will try to borrow the RefCell during iteration

        let inner = self.inner.borrow();
        f.debug_set().entries(inner.slots.iter()).finish()
    }
}

impl<T: fmt::Debug> fmt::Debug for WeakSetSlot<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WeakSetSlot::Empty => write!(f, "empty"),
            WeakSetSlot::Used(ref val, refcount) => write!(f, "{:?}({})", val, refcount)
        }
    }
}

#[test]
fn test_set() {
    let mut set = WeakSet::new();
    let _0 = set.insert("hello world!");
    println!("set: {:?}", set);
    let _1 = set.insert("hello luna!");
    println!("set: {:?}", set);

    drop(_0);
    println!("set: {:?}", set);

    let _2 = set.insert("hello enso!");
    println!("set: {:?}", set);

    let _3 = _2.clone();
    println!("set: {:?}", set);
}