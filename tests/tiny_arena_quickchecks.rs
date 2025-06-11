extern crate typed_generational_arena;
#[macro_use]
extern crate quickcheck;

use std::collections::BTreeSet;
use std::iter::FromIterator;
use typed_generational_arena::TinyArena as Arena;

quickcheck! {
    fn always_contains_inserted_elements(elems: Vec<usize>) -> bool {
        let mut arena = Arena::new();
        let mut elems = elems;
        // Avoid overflow
        elems.truncate((std::u16::MAX - 1) as usize);
        let indices: Vec<_> = elems.into_iter().map(|e| arena.insert(e)).collect();
        indices.into_iter().all(|i| arena.contains(i))
    }
}

quickcheck! {
    fn never_contains_deleted_elements(elems: Vec<usize>) -> bool {
        let mut arena = Arena::new();
        let mut elems = elems;
        // Avoid overflow
        elems.truncate((std::u16::MAX - 1) as usize);
        let indices: Vec<_> = elems.into_iter().map(|e| arena.insert(e)).collect();
        for i in indices.iter().cloned() {
            arena.remove(i).unwrap();
        }
        indices.into_iter().all(|i| !arena.contains(i))
    }
}

quickcheck! {
    fn insert_delete_insert(elems: Vec<usize>) -> bool {
        let mut arena = Arena::new();
        let mut elems = elems;
        // Avoid overflow
        elems.truncate((std::u16::MAX - 1) as usize);

        let indices: Vec<_> = elems.iter().cloned().map(|e| arena.insert(e)).collect();
        for (i, idx) in indices.iter().cloned().enumerate() {
            if arena.remove(idx).unwrap() != elems[i] {
                return false;
            }
        }

        let new_indices: Vec<_> = elems.iter().cloned().map(|e| arena.insert(e)).collect();
        new_indices.into_iter().enumerate().all(|(i, idx)| {
            !arena.contains(indices[i]) && arena.remove(idx).unwrap() == elems[i]
        })
    }
}

quickcheck! {
    fn interp(ops: Vec<(bool, usize)>) -> () {
        let mut arena = Arena::new();
        let mut ops = ops;
        // Avoid overflow
        ops.truncate((std::u16::MAX - 1) as usize);
        let mut live_indices = vec![];
        let mut dead_indices = vec![];

        for (delete, i) in ops {
            if delete && !live_indices.is_empty() {
                let i = i % live_indices.len();
                let (idx, expected) = live_indices.remove(i);
                assert_eq!(arena.remove(idx).unwrap(), expected);
                dead_indices.push(idx);
            } else {
                live_indices.push((arena.insert(i), i));
            }

            // All live indices always have the expected value.
            for (live, expected) in live_indices.iter().cloned() {
                assert_eq!(*arena.get(live).unwrap(), expected);
            }

            // All dead indices are never contained in the arena.
            for dead in dead_indices.iter().cloned() {
                assert!(!arena.contains(dead));
            }
        }

        // All the remaining values are expected.
        let remaining: Vec<_> = arena.into_iter().collect();
        assert_eq!(remaining.len(), live_indices.len());
        for rem in remaining {
            let i = live_indices.iter().position(|&(_, v)| v == rem).unwrap();
            live_indices.remove(i);
        }
    }
}

quickcheck! {
    fn iter(elems: BTreeSet<usize>) -> bool {
        let arena = Arena::from_iter(elems.iter().take((std::u16::MAX - 1) as usize).cloned());
        arena.iter().all(|(idx, value)| {
            elems.contains(value) && arena.get(idx) == Some(value)
        })
    }
}

quickcheck! {
    fn iter_mut(elems: BTreeSet<usize>) -> bool {
        let mut arena = Arena::from_iter(elems.iter().take((std::u16::MAX - 1) as usize).cloned());
        for (_, value) in &mut arena {
            *value = value.wrapping_add(1);
        }
        arena.iter().all(|(idx, value)| {
            let orig_value = value.wrapping_sub(1);
            elems.contains(&orig_value) && arena.get(idx) == Some(value)
        })
    }
}

quickcheck! {
    fn from_iter_into_iter(elems: BTreeSet<usize>) -> bool {
        let arena = Arena::from_iter(elems.iter().take((std::u16::MAX - 1) as usize).cloned());
        arena.into_iter().collect::<BTreeSet<_>>() == elems
    }
}
