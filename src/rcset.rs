use std::rc::{Rc, Weak};
use std::cell::{RefCell, Ref};
use std::collections::hash_map::{HashMap, Values as HashMapValues};
use std::mem::{ManuallyDrop, transmute};
use std::iter::Iterator;

unsafe fn fix_values_lifetime<'a, 'b, T>(values: HashMapValues<'a, *const T, Weak<T>>) -> HashMapValues<'b, *const T, Weak<T>> {
    transmute(values)
}

#[derive(Debug)]
pub struct RcSet<T> {
    inner: Rc<RefCell<HashMap<*const T, Weak<T>>>>
}
impl<T> RcSet<T> {
    pub fn new() -> RcSet<T> {
        RcSet { inner: Rc::new(RefCell::new(HashMap::new())) }
    }
    pub fn insert(&mut self, item: T) -> Item<T> {
        let rc = Rc::new(item);
        let (rc, rc_ptr) = unsafe {
            let ptr = Rc::into_raw(rc);
            let rc = Rc::from_raw(ptr);
            (rc, ptr)
        };
        let weak = Rc::downgrade(&rc);
        self.inner.borrow_mut().insert(rc_ptr, weak);
        
        Item {
            rc: ManuallyDrop::new(rc),
            set: self.clone()
        }
    }
    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        unsafe {
            let inner = self.inner.borrow();
            let values = fix_values_lifetime(inner.values());
            Iter {
                _ref: inner,
                // transmute values to escape the borrow. this could be safe since we keep the Ref alive
                iter: values
            }
        }
    }

    pub fn drop_item(&self, item: Rc<T>) {
        let refcount = Rc::strong_count(&item);
        if refcount <= 1 {
            // get the pointer of the rc
            let rc_ptr = unsafe {
                let ptr = Rc::into_raw(item);
                drop(Rc::from_raw(ptr));
                ptr
            };
            self.inner.borrow_mut().remove(&rc_ptr);
        }
    }
}
impl<T> Clone for RcSet<T> {
    fn clone(&self) -> Self {
        RcSet { inner: self.inner.clone() }
    }
}

pub struct Iter<'a, T> {
    _ref: Ref<'a, HashMap<*const T, Weak<T>>>,
    iter: HashMapValues<'a, *const T, Weak<T>>
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Rc<T>;
    fn next(&mut self) -> Option<Rc<T>> {
        while let Some(weak) = self.iter.next() {
            if let Some(rc) = Weak::upgrade(weak) {
                return Some(rc);
            }
        }
        None
    }
}

#[derive(Clone)]
pub struct Item<T> {
    rc: ManuallyDrop<Rc<T>>,
    set: RcSet<T>
}
impl<T> Drop for Item<T> {
    fn drop(&mut self) {
        unsafe {
            self.set.drop_item(ManuallyDrop::take(&mut self.rc))
        }
    }
}

#[test]
fn test_rcset() {
    let mut set = RcSet::new();
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
    set.iter().for_each(|v| println!("{:?}", v));
}