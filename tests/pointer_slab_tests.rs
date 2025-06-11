extern crate typed_generational_arena;
use std::collections::BTreeSet;
use typed_generational_arena::PtrSlab as Slab;

#[test]
fn can_get_live_value() {
    let mut slab = Slab::with_capacity(1);
    let i = slab.try_insert(42).unwrap();
    assert_eq!(slab[i], 42);
}

#[test]
fn try_insert_when_full() {
    let mut slab = Slab::with_capacity(1);
    slab.try_insert(42).unwrap();
    assert_eq!(slab.try_insert(42).unwrap_err(), 42);
}

#[test]
fn capacity_and_reserve() {
    let mut slab: Slab<usize> = Slab::with_capacity(42);
    assert_eq!(slab.capacity(), 42);
    slab.reserve(10);
    assert_eq!(slab.capacity(), 52);
}

#[test]
fn get_mut() {
    let mut slab = Slab::new();
    let idx = slab.insert(5);
    slab[idx] += 1;
    assert_eq!(slab[idx], 6);
}

#[test]
fn get2_mut() {
    let mut slab = Slab::with_capacity(2);
    let idx1 = slab.insert(0);
    let idx2 = slab.insert(1);
    {
        let (item1, item2) = slab.get2_mut(idx1, idx2);
        assert_eq!(item1, Some(&mut 0));
        assert_eq!(item2, Some(&mut 1));
        *item1.unwrap() = 3;
        *item2.unwrap() = 4;
    }
    assert_eq!(slab[idx1], 3);
    assert_eq!(slab[idx2], 4);
}

#[test]
fn into_iter() {
    let mut slab = Slab::new();
    slab.insert(0);
    slab.insert(1);
    slab.insert(2);
    let set: BTreeSet<_> = slab.into_iter().collect();
    assert_eq!(set.len(), 3);
    assert!(set.contains(&0));
    assert!(set.contains(&1));
    assert!(set.contains(&2));
}

#[test]
fn out_of_bounds_get_with_index_from_other_slab() {
    let mut slab1 = Slab::with_capacity(1);
    let slab2 = Slab::<usize>::with_capacity(1);
    slab1.insert(0);
    let idx = slab1.insert(42);
    assert!(slab2.get(idx).is_none());
}

#[test]
fn out_of_bounds_get2_mut_with_index_from_other_slab() {
    let mut slab1 = Slab::with_capacity(1);
    let mut slab2 = Slab::with_capacity(2);
    let idx1 = slab1.insert(42);
    slab2.insert(0);
    let idx2 = slab2.insert(0);

    assert_eq!(slab1.get2_mut(idx1, idx2), (Some(&mut 42), None));
}

#[test]
fn drain() {
    let mut slab = Slab::new();
    let idx_1 = slab.insert("hello");
    let idx_2 = slab.insert("world");

    assert!(slab.get(idx_1).is_some());
    assert!(slab.get(idx_2).is_some());
    for (idx, value) in slab.drain() {
        assert!((idx == idx_1 && value == "hello") || (idx == idx_2 && value == "world"));
    }
    assert!(slab.get(idx_1).is_none());
    assert!(slab.get(idx_2).is_none());
}

#[test]
fn clear() {
    let mut slab = Slab::with_capacity(1);
    slab.insert(42);
    slab.insert(43);

    assert_eq!(slab.capacity(), 2);
    assert_eq!(slab.len(), 2);

    slab.clear();

    assert_eq!(slab.capacity(), 2);
    assert_eq!(slab.len(), 0);

    slab.insert(44);
    slab.insert(45);
    slab.insert(46);

    assert_eq!(slab.capacity(), 4);
    assert_eq!(slab.len(), 3);

    slab.clear();

    assert_eq!(slab.capacity(), 4);
    assert_eq!(slab.len(), 0);
}
