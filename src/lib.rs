/*!
[![](https://docs.rs/typed-generational-arena/badge.svg)](https://docs.rs/typed-generational-arena/)
[![](https://img.shields.io/crates/v/typed-generational-arena.svg)](https://crates.io/crates/typed-generational-arena)
[![](https://img.shields.io/crates/d/typed-generational-arena.svg)](https://crates.io/crates/typed-generational-arena)

A safe arena allocator that allows deletion without suffering from [the ABA
problem](https://en.wikipedia.org/wiki/ABA_problem) by using generational type-safe
indices. Forked from [generational-arena](https://github.com/fitzgen/generational-arena/).

Inspired by [Catherine West's closing keynote at RustConf
2018](http://rustconf.com/program.html#closingkeynote), where these ideas
were presented in the context of an Entity-Component-System for games
programming.

## What? Why?

Imagine you are working with a graph and you want to add and delete individual
nodes at a time, or you are writing a game and its world consists of many
inter-referencing objects with dynamic lifetimes that depend on user
input. These are situations where matching Rust's ownership and lifetime rules
can get tricky.

It doesn't make sense to use shared ownership with interior mutability (ie
`Rc<RefCell<T>>` or `Arc<Mutex<T>>`) nor borrowed references (ie `&'a T` or `&'a
mut T`) for structures. The cycles rule out reference counted types, and the
required shared mutability rules out borrows. Furthermore, lifetimes are dynamic
and don't follow the borrowed-data-outlives-the-borrower discipline.

In these situations, it is tempting to store objects in a `Vec<T>` and have them
reference each other via their indices. No more borrow checker or ownership
problems! Often, this solution is good enough.

However, now we can't delete individual items from that `Vec<T>` when we no
longer need them, because we end up either

* messing up the indices of every element that follows the deleted one, or

* suffering from the [ABA
  problem](https://en.wikipedia.org/wiki/ABA_problem). To elaborate further, if
  we tried to replace the `Vec<T>` with a `Vec<Option<T>>`, and delete an
  element by setting it to `None`, then we create the possibility for this buggy
  sequence:

    * `obj1` references `obj2` at index `i`

    * someone else deletes `obj2` from index `i`, setting that element to `None`

    * a third thing allocates `obj3`, which ends up at index `i`, because the
      element at that index is `None` and therefore available for allocation

    * `obj1` attempts to get `obj2` at index `i`, but incorrectly is given
      `obj3`, when instead the get should fail.

By introducing a monotonically increasing generation counter to the collection,
associating each element in the collection with the generation when it was
inserted, and getting elements from the collection with the *pair* of index and
the generation at the time when the element was inserted, then we can solve the
aforementioned ABA problem. When indexing into the collection, if the index
pair's generation does not match the generation of the element at that index,
then the operation fails.

## Features

* Zero `unsafe`
* Well tested, including quickchecks
* `no_std` compatibility
* All the trait implementations you expect: `IntoIterator`, `FromIterator`,
  `Extend`, etc...

## Usage

First, add `typed-generational-arena` to your `Cargo.toml`:

```toml
[dependencies]
typed-generational-arena = "0.1"
```

Then, import the crate and use the
[`typed_generational_arena::Arena`](./struct.Arena.html) type!

```rust
extern crate typed_generational_arena;
use typed_generational_arena::Arena;

let mut arena = Arena::new();

// Insert some elements into the arena.
let rza = arena.insert("Robert Fitzgerald Diggs");
let gza = arena.insert("Gary Grice");
let bill = arena.insert("Bill Gates");

// Inserted elements can be accessed infallibly via indexing (and missing
// entries will panic).
assert_eq!(arena[rza], "Robert Fitzgerald Diggs");

// Alternatively, the `get` and `get_mut` methods provide fallible lookup.
if let Some(genius) = arena.get(gza) {
    println!("The gza gza genius: {}", genius);
}
if let Some(val) = arena.get_mut(bill) {
    *val = "Bill Gates doesn't belong in this set...";
}

// We can remove elements.
arena.remove(bill);

// Insert a new one.
let murray = arena.insert("Bill Murray");

// The arena does not contain `bill` anymore, but it does contain `murray`, even
// though they are almost certainly at the same index within the arena in
// practice. Ambiguities are resolved with an associated generation tag.
assert!(!arena.contains(bill));
assert!(arena.contains(murray));

// Iterate over everything inside the arena.
for (idx, value) in &arena {
    println!("{:?} is at {:?}", value, idx);
}
```

## `no_std`

To enable `no_std` compatibility, disable the on-by-default "std" feature. This
currently requires nightly Rust and `feature(alloc)` to get access to `Vec`.

```toml
[dependencies]
typed-generational-arena = { version = "0.1", default-features = false }
```

### Serialization and Deserialization with [`serde`](https://crates.io/crates/serde)

To enable serialization/deserialization support, enable the "serde" feature.

```toml
[dependencies]
typed-generational-arena = { version = "0.1", features = ["serde"] }
```
 */

#![forbid(unsafe_code, missing_docs, missing_debug_implementations)]
#![no_std]
#![cfg_attr(not(feature = "std"), feature(alloc))]

extern crate num_traits;
#[macro_use]
extern crate cfg_if;
#[cfg(feature = "serde")]
extern crate serde;

cfg_if! {
    if #[cfg(feature = "std")] {
        extern crate std;
        use std::vec::{self, Vec};
    } else {
        extern crate alloc;
        use alloc::vec::{self, Vec};
    }
}

use core::cmp;
use core::iter::{self, Extend, FromIterator, FusedIterator};
use core::mem;
use core::ops::{self, AddAssign};
use core::slice;
use core::default::Default;

use num_traits::{ToPrimitive, FromPrimitive};

#[cfg(feature = "serde")]
mod serde_impl;
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

/// A type which can be used as the index of a generation.
pub trait GenerationalIndex : Eq {
    /// Get an object representing the first possible generation
    fn first_generation() -> Self;
    /// Increment the generation of this object. May wrap or panic on overflow depending on type.
    fn increment_generation(&mut self);
}

impl<T: Eq + FromPrimitive + AddAssign + Default> GenerationalIndex for T {
    fn first_generation() -> Self { Default::default() }
    fn increment_generation(&mut self) { *self += Self::from_u8(1).unwrap() }
}

/// If this is used as a generational index, then the arena ignores generation
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct IgnoreGeneration;

impl GenerationalIndex for IgnoreGeneration {
    fn first_generation() -> Self { IgnoreGeneration }
    fn increment_generation(&mut self) {}
}

/// A type which can be used as an index to an arena
pub trait ArenaIndex: ToPrimitive + FromPrimitive {}
impl<T: ToPrimitive + FromPrimitive> ArenaIndex for T {}

/// The `Arena` allows inserting and removing elements that are referred to by
/// `Index`.
///
/// [See the module-level documentation for example usage and motivation.](./index.html)
#[derive(Clone, Debug)]
pub struct Arena<T> {
    // It is a breaking change to modify these three members, as they are needed for serialization
    items: Vec<Entry<T>>,
    generation: u64,
    len: usize,
    free_list_head: Option<usize>,
}

#[derive(Clone, Debug)]
enum Entry<T> {
    Free { next_free: Option<usize> },
    Occupied { generation: u64, value: T },
}

impl<T> Default for Entry<T> {
    fn default() -> Entry<T> {
        Entry::Free { next_free : None }
    }
}

mod delegate_untyped_index {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Index {
        pub index : usize,
        pub generation : u64
    }
}

/// An index (and generation) into an `Arena`.
///
/// To get an `Index`, insert an element into an `Arena`, and the `Index` for
/// that element will be returned.
///
/// # Examples
///
/// ```
/// use typed_generational_arena::Arena;
///
/// let mut arena = Arena::new();
/// let idx = arena.insert(123);
/// assert_eq!(arena[idx], 123);
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Index<T> {
    index: usize,
    generation: u64,
    #[cfg_attr(feature = "serde", serde(skip))]
    _phantom: core::marker::PhantomData<fn() -> T>
}
impl<T> Index<T> {
    #[inline]
    pub(self) fn new(index : usize, generation : u64) -> Index<T> {
        Index{ index : index, generation : generation, _phantom : std::marker::PhantomData }
    }
    #[inline]
    pub(self) fn as_untyped(&self) -> delegate_untyped_index::Index {
        delegate_untyped_index::Index{ index : self.index, generation : self.generation }
    }
}
impl<T> Clone for Index<T> {
    #[inline]
    fn clone(&self) -> Index<T> { Index::new(self.index, self.generation) }
}
impl<T> Copy for Index<T> {}

impl<T> PartialEq for Index<T> {
    #[inline]
    fn eq(&self, other : &Self) -> bool {
        self.as_untyped().eq(&other.as_untyped())
    }
}

impl<T> Eq for Index<T> {}
impl<T> PartialOrd for Index<T> {
    #[inline]
    fn partial_cmp(&self, other : &Self) -> Option<core::cmp::Ordering> {
        self.as_untyped().partial_cmp(&other.as_untyped())
    }
    #[inline]
    fn lt(&self, other : &Self) -> bool {
        self.as_untyped().lt(&other.as_untyped())
    }
    #[inline]
    fn le(&self, other : &Self) -> bool {
        self.as_untyped().le(&other.as_untyped())
    }
    #[inline]
    fn gt(&self, other : &Self) -> bool {
        self.as_untyped().gt(&other.as_untyped())
    }
    #[inline]
    fn ge(&self, other : &Self) -> bool {
        self.as_untyped().ge(&other.as_untyped())
    }
}

impl<T> Ord for Index<T> {
    fn cmp(&self, other : &Self) -> core::cmp::Ordering {
        self.as_untyped().cmp(&other.as_untyped())
    }
}

impl<T> core::hash::Hash for Index<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state : &mut H) {
        self.as_untyped().hash(state)
    }
}
impl<T> core::fmt::Debug for Index<T> {
    #[inline]
    fn fmt(&self, f : &mut core::fmt::Formatter) -> core::fmt::Result {
        self.as_untyped().fmt(f)
    }
}

const DEFAULT_CAPACITY: usize = 4;

impl<T> Arena<T> {
    /// Constructs a new, empty `Arena`.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::<usize>::new();
    /// # let _ = arena;
    /// ```
    pub fn new() -> Arena<T> {
        Arena::with_capacity(DEFAULT_CAPACITY)
    }

    /// Constructs a new, empty `Arena<T>` with the specified capacity.
    ///
    /// The `Arena<T>` will be able to hold `n` elements without further allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::with_capacity(10);
    ///
    /// // These insertions will not require further allocation.
    /// for i in 0..10 {
    ///     assert!(arena.try_insert(i).is_ok());
    /// }
    ///
    /// // But now we are at capacity, and there is no more room.
    /// assert!(arena.try_insert(99).is_err());
    /// ```
    pub fn with_capacity(n: usize) -> Arena<T> {
        let n = cmp::max(n, 1);
        let mut arena = Arena {
            items: Vec::new(),
            generation: 0,
            free_list_head: None,
            len: 0,
        };
        arena.reserve(n);
        arena
    }

    /// Clear all the items inside the arena, but keep its allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::with_capacity(1);
    /// arena.insert(42);
    /// arena.insert(43);
    ///
    /// arena.clear();
    ///
    /// assert_eq!(arena.capacity(), 2);
    /// ```
    pub fn clear(&mut self) {
        self.items.clear();

        let end = self.items.capacity();
        self.items.extend((0..end).map(|i| {
            if i == end - 1 {
                Entry::Free { next_free: None }
            } else {
                Entry::Free {
                    next_free: Some(i + 1),
                }
            }
        }));
        self.free_list_head = Some(0);
        self.len = 0;
    }

    /// Attempts to insert `value` into the arena using existing capacity.
    ///
    /// This method will never allocate new capacity in the arena.
    ///
    /// If insertion succeeds, then the `value`'s index is returned. If
    /// insertion fails, then `Err(value)` is returned to give ownership of
    /// `value` back to the caller.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    ///
    /// match arena.try_insert(42) {
    ///     Ok(idx) => {
    ///         // Insertion succeeded.
    ///         assert_eq!(arena[idx], 42);
    ///     }
    ///     Err(x) => {
    ///         // Insertion failed.
    ///         assert_eq!(x, 42);
    ///     }
    /// };
    /// ```
    #[inline]
    pub fn try_insert(&mut self, value: T) -> Result<Index<T>, T> {
        match self.free_list_head {
            None => Err(value),
            Some(i) => match self.items[i] {
                Entry::Occupied { .. } => panic!("corrupt free list"),
                Entry::Free { next_free } => {
                    self.free_list_head = next_free;
                    self.len += 1;
                    self.items[i] = Entry::Occupied {
                        generation: self.generation,
                        value,
                    };
                    Ok(Index::new(i, self.generation))
                }
            },
        }
    }

    /// Insert `value` into the arena, allocating more capacity if necessary.
    ///
    /// The `value`'s associated index in the arena is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    ///
    /// let idx = arena.insert(42);
    /// assert_eq!(arena[idx], 42);
    /// ```
    #[inline]
    pub fn insert(&mut self, value: T) -> Index<T> {
        match self.try_insert(value) {
            Ok(i) => i,
            Err(value) => self.insert_slow_path(value),
        }
    }

    #[inline(never)]
    fn insert_slow_path(&mut self, value: T) -> Index<T> {
        let len = self.items.len();
        self.reserve(len);
        self.try_insert(value)
            .map_err(|_| ())
            .expect("inserting will always succeed after reserving additional space")
    }

    /// Remove the element at index `i` from the arena.
    ///
    /// If the element at index `i` is still in the arena, then it is
    /// returned. If it is not in the arena, then `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let idx = arena.insert(42);
    ///
    /// assert_eq!(arena.remove(idx), Some(42));
    /// assert_eq!(arena.remove(idx), None);
    /// ```
    pub fn remove(&mut self, i: Index<T>) -> Option<T> {
        if i.index >= self.items.len() {
            return None;
        }

        let entry = mem::replace(
            &mut self.items[i.index],
            Entry::Free {
                next_free: self.free_list_head,
            },
        );
        match entry {
            Entry::Occupied { generation, value } => {
                if generation == i.generation {
                    self.generation += 1;
                    self.free_list_head = Some(i.index);
                    self.len -= 1;
                    Some(value)
                } else {
                    self.items[i.index] = Entry::Occupied { generation, value };
                    None
                }
            }
            e @ Entry::Free { .. } => {
                self.items[i.index] = e;
                None
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all indices such that `predicate(index, &value)` returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut crew = Arena::new();
    /// crew.extend(&["Jim Hawkins", "John Silver", "Alexander Smollett", "Israel Hands"]);
    /// let pirates = ["John Silver", "Israel Hands"]; // too dangerous to keep them around
    /// crew.retain(|_index, member| !pirates.contains(member));
    /// let mut crew_members = crew.iter().map(|(_, member)| **member);
    /// assert_eq!(crew_members.next(), Some("Jim Hawkins"));
    /// assert_eq!(crew_members.next(), Some("Alexander Smollett"));
    /// assert!(crew_members.next().is_none());
    /// ```
    pub fn retain(&mut self, mut predicate: impl FnMut(Index<T>, &T) -> bool) {
        for i in 0..self.len {
            let remove = match &self.items[i] {
                Entry::Occupied { generation, value } => {
                    let index = Index::new(i, *generation);
                    if predicate(index, value) {
                        None
                    } else {
                        Some(index)
                    }
                }

                _ => None,
            };
            if let Some(index) = remove {
                self.remove(index);
            }
        }
    }

    /// Is the element at index `i` in the arena?
    ///
    /// Returns `true` if the element at `i` is in the arena, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let idx = arena.insert(42);
    ///
    /// assert!(arena.contains(idx));
    /// arena.remove(idx);
    /// assert!(!arena.contains(idx));
    /// ```
    pub fn contains(&self, i: Index<T>) -> bool {
        self.get(i).is_some()
    }

    /// Get a shared reference to the element at index `i` if it is in the
    /// arena.
    ///
    /// If the element at index `i` is not in the arena, then `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let idx = arena.insert(42);
    ///
    /// assert_eq!(arena.get(idx), Some(&42));
    /// arena.remove(idx);
    /// assert!(arena.get(idx).is_none());
    /// ```
    pub fn get(&self, i: Index<T>) -> Option<&T> {
        match self.items.get(i.index) {
            Some(Entry::Occupied {
                generation,
                ref value,
            }) if *generation == i.generation => Some(value),
            _ => None,
        }
    }

    /// Get an exclusive reference to the element at index `i` if it is in the
    /// arena.
    ///
    /// If the element at index `i` is not in the arena, then `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let idx = arena.insert(42);
    ///
    /// *arena.get_mut(idx).unwrap() += 1;
    /// assert_eq!(arena.remove(idx), Some(43));
    /// assert!(arena.get_mut(idx).is_none());
    /// ```
    pub fn get_mut(&mut self, i: Index<T>) -> Option<&mut T> {
        match self.items.get_mut(i.index) {
            Some(Entry::Occupied {
                generation,
                ref mut value,
            }) if *generation == i.generation => Some(value),
            _ => None,
        }
    }

    /// Get a pair of exclusive references to the elements at index `i1` and `i2` if it is in the
    /// arena.
    ///
    /// If the element at index `i1` or `i2` is not in the arena, then `None` is returned for this
    /// element.
    ///
    /// # Panics
    ///
    /// Panics if `i1` and `i2` are pointing to the same item of the arena.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let idx1 = arena.insert(0);
    /// let idx2 = arena.insert(1);
    ///
    /// {
    ///     let (item1, item2) = arena.get2_mut(idx1, idx2);
    ///
    ///     *item1.unwrap() = 3;
    ///     *item2.unwrap() = 4;
    /// }
    ///
    /// assert_eq!(arena[idx1], 3);
    /// assert_eq!(arena[idx2], 4);
    /// ```
    pub fn get2_mut(&mut self, i1: Index<T>, i2: Index<T>) -> (Option<&mut T>, Option<&mut T>) {
        let len = self.items.len();

        if i1.index == i2.index {
            assert!(i1.generation != i2.generation);

            if i1.generation > i2.generation {
                return (self.get_mut(i1), None);
            }
            return (None, self.get_mut(i2));
        }

        if i1.index >= len {
            return (None, self.get_mut(i2));
        } else if i2.index >= len {
            return (self.get_mut(i1), None);
        }

        let (raw_item1, raw_item2) = {
            let (xs, ys) = self.items.split_at_mut(cmp::max(i1.index, i2.index));
            if i1.index < i2.index {
                (&mut xs[i1.index], &mut ys[0])
            } else {
                (&mut ys[0], &mut xs[i2.index])
            }
        };

        let item1 = match raw_item1 {
            Entry::Occupied {
                generation,
                ref mut value,
            } if *generation == i1.generation => Some(value),
            _ => None,
        };

        let item2 = match raw_item2 {
            Entry::Occupied {
                generation,
                ref mut value,
            } if *generation == i2.generation => Some(value),
            _ => None,
        };

        (item1, item2)
    }

    /// Get the length of this arena.
    ///
    /// The length is the number of elements the arena holds.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// assert_eq!(arena.len(), 0);
    ///
    /// let idx = arena.insert(42);
    /// assert_eq!(arena.len(), 1);
    ///
    /// let _ = arena.insert(0);
    /// assert_eq!(arena.len(), 2);
    ///
    /// assert_eq!(arena.remove(idx), Some(42));
    /// assert_eq!(arena.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the arena contains no elements
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// assert!(arena.is_empty());
    ///
    /// let idx = arena.insert(42);
    /// assert!(!arena.is_empty());
    ///
    /// assert_eq!(arena.remove(idx), Some(42));
    /// assert!(arena.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the capacity of this arena.
    ///
    /// The capacity is the maximum number of elements the arena can hold
    /// without further allocation, including however many it currently
    /// contains.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::with_capacity(10);
    /// assert_eq!(arena.capacity(), 10);
    ///
    /// // `try_insert` does not allocate new capacity.
    /// for i in 0..10 {
    ///     assert!(arena.try_insert(1).is_ok());
    ///     assert_eq!(arena.capacity(), 10);
    /// }
    ///
    /// // But `insert` will if the arena is already at capacity.
    /// arena.insert(0);
    /// assert!(arena.capacity() > 10);
    /// ```
    pub fn capacity(&self) -> usize {
        self.items.len()
    }

    /// Allocate space for `additional_capacity` more elements in the arena.
    ///
    /// # Panics
    ///
    /// Panics if this causes the capacity to overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::with_capacity(10);
    /// arena.reserve(5);
    /// assert_eq!(arena.capacity(), 15);
    /// # let _: Arena<usize> = arena;
    /// ```
    pub fn reserve(&mut self, additional_capacity: usize) {
        let start = self.items.len();
        let end = self.items.len() + additional_capacity;
        let old_head = self.free_list_head;
        self.items.reserve_exact(additional_capacity);
        self.items.extend((start..end).map(|i| {
            if i == end - 1 {
                Entry::Free {
                    next_free: old_head,
                }
            } else {
                Entry::Free {
                    next_free: Some(i + 1),
                }
            }
        }));
        self.free_list_head = Some(start);
    }

    /// Iterate over shared references to the elements in this arena.
    ///
    /// Yields pairs of `(Index<T>, &T)` items.
    ///
    /// Order of iteration is not defined.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// for i in 0..10 {
    ///     arena.insert(i * i);
    /// }
    ///
    /// for (idx, value) in arena.iter() {
    ///     println!("{} is at index {:?}", value, idx);
    /// }
    /// ```
    pub fn iter(&self) -> Iter<T> {
        Iter {
            len: self.len,
            inner: self.items.iter().enumerate(),
        }
    }

    /// Iterate over exclusive references to the elements in this arena.
    ///
    /// Yields pairs of `(Index<T>, &mut T)` items.
    ///
    /// Order of iteration is not defined.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// for i in 0..10 {
    ///     arena.insert(i * i);
    /// }
    ///
    /// for (_idx, value) in arena.iter_mut() {
    ///     *value += 5;
    /// }
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            len: self.len,
            inner: self.items.iter_mut().enumerate(),
        }
    }

    /// Iterate over elements of the arena and remove them.
    ///
    /// Yields pairs of `(Index<T>, T)` items.
    ///
    /// Order of iteration is not defined.
    ///
    /// Note: All elements are removed even if the iterator is only partially consumed or not consumed at all.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let idx_1 = arena.insert("hello");
    /// let idx_2 = arena.insert("world");
    ///
    /// assert!(arena.get(idx_1).is_some());
    /// assert!(arena.get(idx_2).is_some());
    /// for (idx, value) in arena.drain() {
    ///     assert!((idx == idx_1 && value == "hello") || (idx == idx_2 && value == "world"));
    /// }
    /// assert!(arena.get(idx_1).is_none());
    /// assert!(arena.get(idx_2).is_none());
    /// ```
    pub fn drain(&mut self) -> Drain<T> {
        Drain {
            inner: self.items.drain(..).enumerate(),
        }
    }
}

impl<T> IntoIterator for Arena<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            len: self.len,
            inner: self.items.into_iter(),
        }
    }
}

/// An iterator over the elements in an arena.
///
/// Yields `T` items.
///
/// Order of iteration is not defined.
///
/// # Examples
///
/// ```
/// use typed_generational_arena::Arena;
///
/// let mut arena = Arena::new();
/// for i in 0..10 {
///     arena.insert(i * i);
/// }
///
/// for value in arena {
///     assert!(value < 100);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct IntoIter<T> {
    len: usize,
    inner: vec::IntoIter<Entry<T>>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Entry::Free { .. }) => continue,
                Some(Entry::Occupied { value, .. }) => {
                    self.len -= 1;
                    return Some(value);
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some(Entry::Free { .. }) => continue,
                Some(Entry::Occupied { value, .. }) => {
                    self.len -= 1;
                    return Some(value);
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<T> FusedIterator for IntoIter<T> {}

impl<'a, T> IntoIterator for &'a Arena<T> {
    type Item = (Index<T>, &'a T);
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator over shared references to the elements in an arena.
///
/// Yields pairs of `(Index<T>, &T)` items.
///
/// Order of iteration is not defined.
///
/// # Examples
///
/// ```
/// use typed_generational_arena::Arena;
///
/// let mut arena = Arena::new();
/// for i in 0..10 {
///     arena.insert(i * i);
/// }
///
/// for (idx, value) in &arena {
///     println!("{} is at index {:?}", value, idx);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Iter<'a, T: 'a> {
    len: usize,
    inner: iter::Enumerate<slice::Iter<'a, Entry<T>>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Index<T>, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, &Entry::Free { .. })) => continue,
                Some((
                    index,
                    &Entry::Occupied {
                        generation,
                        ref value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Index::new(index, generation);
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, &Entry::Free { .. })) => continue,
                Some((
                    index,
                    &Entry::Occupied {
                        generation,
                        ref value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Index::new(index, generation);
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

impl<'a, T> IntoIterator for &'a mut Arena<T> {
    type Item = (Index<T>, &'a mut T);
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An iterator over exclusive references to elements in this arena.
///
/// Yields pairs of `(Index<T>, &mut T)` items.
///
/// Order of iteration is not defined.
///
/// # Examples
///
/// ```
/// use typed_generational_arena::Arena;
///
/// let mut arena = Arena::new();
/// for i in 0..10 {
///     arena.insert(i * i);
/// }
///
/// for (_idx, value) in &mut arena {
///     *value += 5;
/// }
/// ```
#[derive(Debug)]
pub struct IterMut<'a, T: 'a> {
    len: usize,
    inner: iter::Enumerate<slice::IterMut<'a, Entry<T>>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Index<T>, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, &mut Entry::Free { .. })) => continue,
                Some((
                    index,
                    &mut Entry::Occupied {
                        generation,
                        ref mut value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Index::new(index, generation);
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, &mut Entry::Free { .. })) => continue,
                Some((
                    index,
                    &mut Entry::Occupied {
                        generation,
                        ref mut value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Index::new(index, generation);
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

/// An iterator that removes elements from the arena.
///
/// Yields pairs of `(Index<T>, T)` items.
///
/// Order of iteration is not defined.
///
/// Note: All elements are removed even if the iterator is only partially consumed or not consumed at all.
///
/// # Examples
///
/// ```
/// use typed_generational_arena::Arena;
///
/// let mut arena = Arena::new();
/// let idx_1 = arena.insert("hello");
/// let idx_2 = arena.insert("world");
///
/// assert!(arena.get(idx_1).is_some());
/// assert!(arena.get(idx_2).is_some());
/// for (idx, value) in arena.drain() {
///     assert!((idx == idx_1 && value == "hello") || (idx == idx_2 && value == "world"));
/// }
/// assert!(arena.get(idx_1).is_none());
/// assert!(arena.get(idx_2).is_none());
/// ```
#[derive(Debug)]
pub struct Drain<'a, T: 'a> {
    inner: iter::Enumerate<vec::Drain<'a, Entry<T>>>,
}

impl<'a, T> Iterator for Drain<'a, T> {
    type Item = (Index<T>, T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, Entry::Free { .. })) => continue,
                Some((index, Entry::Occupied { generation, value })) => {
                    let idx = Index::new(index, generation);
                    return Some((idx, value));
                }
                None => return None,
            }
        }
    }
}

impl<T> Extend<T> for Arena<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<T> FromIterator<T> for Arena<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let cap = upper.unwrap_or(lower);
        let cap = cmp::max(cap, 1);
        let mut arena = Arena::with_capacity(cap);
        arena.extend(iter);
        arena
    }
}

impl<T> ops::Index<Index<T>> for Arena<T> {
    type Output = T;

    fn index(&self, index: Index<T>) -> &Self::Output {
        self.get(index).expect("No element at index")
    }
}

impl<T> ops::IndexMut<Index<T>> for Arena<T> {
    fn index_mut(&mut self, index: Index<T>) -> &mut Self::Output {
        self.get_mut(index).expect("No element at index")
    }
}
