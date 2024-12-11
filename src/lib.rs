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
typed-generational-arena = "0.2"
```

Then, import the crate and use one of the variations of the
[`typed_generational_arena::Arena`](./struct.Arena.html) type!
In these examples, we use `typed_generational_arena::StandardArena`,
but you can use any combination of index and generation ID
best fits your purposes.

```rust
extern crate typed_generational_arena;
use typed_generational_arena::StandardArena;

let mut arena = StandardArena::new();

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
typed-generational-arena = { version = "0.2", default-features = false }
```

### Serialization and Deserialization with [`serde`](https://crates.io/crates/serde)

To enable serialization/deserialization support, enable the "serde" feature.

```toml
[dependencies]
typed-generational-arena = { version = "0.2", features = ["serde"] }
```
 */

#![forbid(unsafe_code, missing_docs, missing_debug_implementations)]
#![no_std]
#![cfg_attr(not(feature = "std"), feature(alloc))]

extern crate nonzero_ext;
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

use core::cmp::{self, Ordering};
use core::default::Default;
use core::fmt::Debug;
use core::hash::Hash;
use core::iter::{self, Extend, FromIterator, FusedIterator};
use core::mem;
use core::ops::{self, Add, AddAssign};
use core::slice;

use nonzero_ext::{NonZero, NonZeroAble};
use num_traits::{FromPrimitive, One, ToPrimitive, WrappingAdd, Zero};

#[cfg(feature = "serde")]
mod serde_impl;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

mod presets;
pub use presets::*;

/// A type which can be used as the index of a generation which may not be able to be incremented
pub trait FixedGenerationalIndex: Copy + Eq {
    /// Get an object representing the first possible generation
    fn first_generation() -> Self;
    /// Compare this generation with another.
    fn generation_lt(&self, other: &Self) -> bool;
}

/// A type which can be used as the index of a generation, which can be incremented
pub trait GenerationalIndex: FixedGenerationalIndex {
    /// Increment the generation of this object. May wrap or panic on overflow depending on type.
    fn increment_generation(&mut self);
}

/// A generation counter which is always nonzero. Useful for size optimizations on Option<Index>
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NonzeroGeneration<T: NonZeroAble> {
    gen: T::NonZero,
}

impl<T> FixedGenerationalIndex for NonzeroGeneration<T>
where
    T: NonZeroAble
        + One
        + Add<Output = T>
        + Copy
        + Eq
        + From<<<T as NonZeroAble>::NonZero as NonZero>::Primitive>,
    T::NonZero: PartialOrd + Eq + Copy,
{
    #[inline(always)]
    fn first_generation() -> Self {
        NonzeroGeneration {
            gen: T::one().as_nonzero().unwrap(),
        }
    }
    #[inline(always)]
    fn generation_lt(&self, other: &Self) -> bool {
        self.gen < other.gen
    }
}

impl<T> GenerationalIndex for NonzeroGeneration<T>
where
    T: NonZeroAble
        + One
        + Add<Output = T>
        + Copy
        + Eq
        + From<<<T as NonZeroAble>::NonZero as NonZero>::Primitive>,
    T::NonZero: PartialOrd + Eq + Copy,
{
    #[inline(always)]
    fn increment_generation(&mut self) {
        self.gen = (T::from(self.gen.get()) + T::one()).as_nonzero().unwrap()
    }
}

/// A wrapping generation counter which is always nonzero.
/// Useful for size optimizations on Option<Index>
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NonzeroWrapGeneration<T: NonZeroAble> {
    gen: T::NonZero,
}

impl<T> FixedGenerationalIndex for NonzeroWrapGeneration<T>
where
    T: NonZeroAble
        + One
        + Zero
        + Copy
        + Eq
        + WrappingAdd
        + From<<<T as NonZeroAble>::NonZero as NonZero>::Primitive>,
    T::NonZero: PartialOrd + Eq + Copy,
{
    #[inline(always)]
    fn first_generation() -> Self {
        NonzeroWrapGeneration {
            gen: T::one().as_nonzero().unwrap(),
        }
    }
    #[inline(always)]
    fn generation_lt(&self, other: &Self) -> bool {
        self.gen < other.gen
    }
}

impl<T> GenerationalIndex for NonzeroWrapGeneration<T>
where
    T: NonZeroAble
        + One
        + Zero
        + Copy
        + Eq
        + WrappingAdd
        + From<<<T as NonZeroAble>::NonZero as NonZero>::Primitive>,
    T::NonZero: PartialOrd + Eq + Copy,
{
    #[inline(always)]
    fn increment_generation(&mut self) {
        let new = T::from(self.gen.get()).wrapping_add(&T::one());
        self.gen = if T::zero() == new {
            Self::first_generation().gen
        } else {
            new.as_nonzero().unwrap()
        }
    }
}

impl<T: Eq + One + AddAssign + Default + PartialOrd + Copy> FixedGenerationalIndex for T {
    #[inline(always)]
    fn first_generation() -> Self {
        Default::default()
    }
    #[inline(always)]
    fn generation_lt(&self, other: &Self) -> bool {
        self.lt(other)
    }
}

impl<T: Eq + One + AddAssign + Default + PartialOrd + Copy> GenerationalIndex for T {
    #[inline(always)]
    fn increment_generation(&mut self) {
        *self += Self::one()
    }
}

/// If this is used as a generational index, then the arena ignores generation
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct IgnoreGeneration;

impl FixedGenerationalIndex for IgnoreGeneration {
    #[inline(always)]
    fn first_generation() -> Self {
        IgnoreGeneration
    }
    #[inline(always)]
    fn generation_lt(&self, _other: &Self) -> bool {
        false
    }
}

impl GenerationalIndex for IgnoreGeneration {
    #[inline(always)]
    fn increment_generation(&mut self) {}
}

/// A marker trait which says that a generation type is ignored
pub trait IgnoredGeneration: FixedGenerationalIndex {}
impl IgnoredGeneration for IgnoreGeneration {}

/// If this is used as a generational index, then the arena is no longer generational
/// and does not allow element removal
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DisableRemoval;

impl FixedGenerationalIndex for DisableRemoval {
    #[inline(always)]
    fn first_generation() -> Self {
        DisableRemoval
    }
    #[inline(always)]
    fn generation_lt(&self, _other: &Self) -> bool {
        false
    }
}

impl IgnoredGeneration for DisableRemoval {}

/// A type which can be used as an index to an arena
pub trait ArenaIndex: Copy {
    /// Create an arena index from a usize
    fn from_idx(idx: usize) -> Self;
    /// Transform an arena index into a usize
    fn to_idx(self) -> usize;
}
impl<T: ToPrimitive + FromPrimitive + Copy> ArenaIndex for T {
    #[inline(always)]
    fn from_idx(idx: usize) -> Self {
        Self::from_usize(idx).unwrap()
    }
    #[inline(always)]
    fn to_idx(self) -> usize {
        self.to_usize().unwrap()
    }
}

/// An arena index which is always nonzero. Useful for Option<T> size optimizations
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NonZeroIndex<T: NonZeroAble> {
    idx: T::NonZero,
}

impl<T> ArenaIndex for NonZeroIndex<T>
where
    T: NonZeroAble + FromPrimitive,
    NonZeroIndex<T>: Copy,
    <<T as NonZeroAble>::NonZero as NonZero>::Primitive: ToPrimitive,
{
    #[inline(always)]
    fn from_idx(idx: usize) -> Self {
        NonZeroIndex {
            idx: T::from_usize(idx + 1).unwrap().as_nonzero().unwrap(),
        }
    }
    #[inline(always)]
    fn to_idx(self) -> usize {
        self.idx.get().to_usize().unwrap() - 1
    }
}

/// The `Arena` allows inserting and removing elements that are referred to by
/// `Index`.
///
/// [See the module-level documentation for example usage and motivation.](./index.html)
#[derive(Clone, Debug)]
pub struct Arena<T, I = usize, G = usize> {
    // It is a breaking change to modify these three members, as they are needed for serialization
    items: Vec<Entry<T, I, G>>,
    generation: G,
    len: usize,
    free_list_head: Option<I>,
}

#[derive(Clone, Debug)]
enum Entry<T, I = usize, G = u64> {
    Free { next_free: Option<I> },
    Occupied { generation: G, value: T },
}

/// An index (and generation) into an `Arena`.
///
/// To get an `Index`, insert an element into an `Arena`, and the `Index` for
/// that element will be returned.
///
/// # Examples
///
/// ```
/// use typed_generational_arena::StandardArena;
///
/// let mut arena = StandardArena::new();
/// let idx = arena.insert(123);
/// assert_eq!(arena[idx], 123);
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Index<T, I = usize, G = u64> {
    /// The array index of the given value
    index: I,
    /// The generation of the given value
    generation: G,
    #[cfg_attr(feature = "serde", serde(skip))]
    _phantom: core::marker::PhantomData<fn() -> T>,
}

impl<T, I: ArenaIndex + Copy, G: FixedGenerationalIndex + Copy> Index<T, I, G> {
    /// Get this index as a `usize`
    pub fn to_idx(&self) -> usize {
        self.index.to_idx()
    }
    /// Get this index's array index into the arena
    pub fn arr_idx(&self) -> I {
        self.index
    }
    /// Get this index's generation
    pub fn gen(&self) -> G {
        self.generation
    }
}

impl<T, I: ArenaIndex + Copy, G: FixedGenerationalIndex> Index<T, I, G> {
    /// Convert a `usize` to an index at the first generation
    #[inline]
    pub fn from_idx_first_gen(n: usize) -> Self {
        Index {
            index: I::from_idx(n),
            generation: G::first_generation(),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<T, I: Debug, G: Debug> Debug for Index<T, I, G> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Index")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}

impl<T, I: Copy, G: Copy> Copy for Index<T, I, G> {}

impl<T, I: Clone, G: Clone> Clone for Index<T, I, G> {
    fn clone(&self) -> Self {
        Index {
            index: self.index.clone(),
            generation: self.generation.clone(),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<T, I: Eq, G: Eq> Eq for Index<T, I, G> {}

impl<T, I: PartialEq, G: PartialEq> PartialEq for Index<T, I, G> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T, I: Hash, G: Hash> Hash for Index<T, I, G> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T, I: ArenaIndex + Copy, G: IgnoredGeneration> Index<T, I, G> {
    /// Convert a `usize` to an index (with generations ignored)
    #[inline(always)]
    pub fn from_idx(n: usize) -> Self {
        Self::from_idx_first_gen(n)
    }
}

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> Index<T, I, G> {
    /// Create a new index from a given array index and generation
    #[inline]
    pub fn new(index: I, generation: G) -> Index<T, I, G> {
        Index {
            index: index,
            generation: generation,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, I: PartialOrd, G: FixedGenerationalIndex> PartialOrd for Index<T, I, G> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.index.partial_cmp(&other.index) {
            Some(ordering) => {
                if ordering == Ordering::Equal {
                    if self.generation.generation_lt(&other.generation) {
                        Some(Ordering::Less)
                    } else if self.generation == other.generation {
                        Some(Ordering::Equal)
                    } else {
                        Some(Ordering::Greater)
                    }
                } else {
                    Some(ordering)
                }
            }
            None => {
                if self.generation.generation_lt(&other.generation) {
                    Some(Ordering::Less)
                } else if self.generation == other.generation {
                    None
                } else {
                    Some(Ordering::Greater)
                }
            }
        }
    }
}

impl<T, I: Ord, G: FixedGenerationalIndex> Ord for Index<T, I, G> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

const DEFAULT_CAPACITY: usize = 4;

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> Arena<T, I, G> {
    /// Constructs a new, empty `Arena`.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::<usize>::new();
    /// # let _ = arena;
    /// ```
    pub fn new() -> Arena<T, I, G> {
        Arena::with_capacity(DEFAULT_CAPACITY)
    }

    /// Constructs a new, empty `Arena<T>` with the specified capacity.
    ///
    /// The `Arena<T>` will be able to hold `n` elements without further allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::with_capacity(10);
    ///
    /// // These insertions will not require further allocation.
    /// for i in 0..10 {
    ///     assert!(arena.try_insert(i).is_ok());
    /// }
    ///
    /// // But now we are at capacity, and there is no more room.
    /// assert!(arena.try_insert(99).is_err());
    /// ```
    pub fn with_capacity(n: usize) -> Arena<T, I, G> {
        let n = cmp::max(n, 1);
        let mut arena = Arena {
            items: Vec::new(),
            generation: G::first_generation(),
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::with_capacity(1);
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
                    next_free: Some(I::from_idx(i + 1)),
                }
            }
        }));
        self.free_list_head = Some(I::from_idx(0));
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
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
    pub fn try_insert(&mut self, value: T) -> Result<Index<T, I, G>, T> {
        match self.free_list_head {
            None => Err(value),
            Some(i) => {
                let idx = i.to_idx();
                match &self.items[idx] {
                    Entry::Occupied { .. } => panic!("corrupt free list"),
                    Entry::Free { next_free } => {
                        self.free_list_head = *next_free;
                        self.len += 1;
                        self.items[idx] = Entry::Occupied {
                            generation: self.generation,
                            value,
                        };
                        Ok(Index::new(i, self.generation))
                    }
                }
            }
        }
    }

    /// Insert `value` into the arena, allocating more capacity if necessary.
    ///
    /// The `value`'s associated index in the arena is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    ///
    /// let idx = arena.insert(42);
    /// assert_eq!(arena[idx], 42);
    /// ```
    #[inline]
    pub fn insert(&mut self, value: T) -> Index<T, I, G> {
        match self.try_insert(value) {
            Ok(i) => i,
            Err(value) => self.insert_slow_path(value),
        }
    }

    #[inline(never)]
    fn insert_slow_path(&mut self, value: T) -> Index<T, I, G> {
        let len = self.items.len();
        self.reserve(len);
        self.try_insert(value)
            .map_err(|_| ())
            .expect("inserting will always succeed after reserving additional space")
    }

    /// Is the element at index `i` in the arena?
    ///
    /// Returns `true` if the element at `i` is in the arena, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    /// let idx = arena.insert(42);
    ///
    /// assert!(arena.contains(idx));
    /// arena.remove(idx);
    /// assert!(!arena.contains(idx));
    /// ```
    pub fn contains(&self, i: Index<T, I, G>) -> bool {
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    /// let idx = arena.insert(42);
    ///
    /// assert_eq!(arena.get(idx), Some(&42));
    /// arena.remove(idx);
    /// assert!(arena.get(idx).is_none());
    /// ```
    pub fn get(&self, i: Index<T, I, G>) -> Option<&T> {
        match self.items.get(i.index.to_idx()) {
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    /// let idx = arena.insert(42);
    ///
    /// *arena.get_mut(idx).unwrap() += 1;
    /// assert_eq!(arena.remove(idx), Some(43));
    /// assert!(arena.get_mut(idx).is_none());
    /// ```
    pub fn get_mut(&mut self, i: Index<T, I, G>) -> Option<&mut T> {
        match self.items.get_mut(i.index.to_idx()) {
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
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
    pub fn get2_mut(
        &mut self,
        i1: Index<T, I, G>,
        i2: Index<T, I, G>,
    ) -> (Option<&mut T>, Option<&mut T>) {
        let len = self.items.len();
        let idx = (i1.index.to_idx(), i2.index.to_idx());
        let gen = (i1.generation, i2.generation);

        if idx.0 == idx.1 {
            assert!(gen.0 != gen.1);

            if gen.1.generation_lt(&gen.0) {
                return (self.get_mut(i1), None);
            }
            return (None, self.get_mut(i2));
        }

        if idx.0 >= len {
            return (None, self.get_mut(i2));
        } else if idx.1 >= len {
            return (self.get_mut(i1), None);
        }

        let (raw_item1, raw_item2) = {
            let (xs, ys) = self.items.split_at_mut(cmp::max(idx.0, idx.1));
            if idx.0 < idx.1 {
                (&mut xs[idx.0], &mut ys[0])
            } else {
                (&mut ys[0], &mut xs[idx.1])
            }
        };

        let item1 = match raw_item1 {
            Entry::Occupied {
                generation,
                ref mut value,
            } if *generation == gen.0 => Some(value),
            _ => None,
        };

        let item2 = match raw_item2 {
            Entry::Occupied {
                generation,
                ref mut value,
            } if *generation == gen.1 => Some(value),
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::with_capacity(10);
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::with_capacity(10);
    /// arena.reserve(5);
    /// assert_eq!(arena.capacity(), 15);
    /// # let _: StandardArena<usize> = arena;
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
                    next_free: Some(I::from_idx(i + 1)),
                }
            }
        }));
        self.free_list_head = Some(I::from_idx(start));
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    /// for i in 0..10 {
    ///     arena.insert(i * i);
    /// }
    ///
    /// for (idx, value) in arena.iter() {
    ///     println!("{} is at index {:?}", value, idx);
    /// }
    /// ```
    pub fn iter(&self) -> Iter<T, I, G> {
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    /// for i in 0..10 {
    ///     arena.insert(i * i);
    /// }
    ///
    /// for (_idx, value) in arena.iter_mut() {
    ///     *value += 5;
    /// }
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<T, I, G> {
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
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
    pub fn drain(&mut self) -> Drain<T, I, G> {
        Drain {
            inner: self.items.drain(..).enumerate(),
        }
    }
}

impl<T, I: ArenaIndex, G: GenerationalIndex> Arena<T, I, G> {
    /// Remove the element at index `i` from the arena.
    ///
    /// If the element at index `i` is still in the arena, then it is
    /// returned. If it is not in the arena, then `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut arena = StandardArena::new();
    /// let idx = arena.insert(42);
    ///
    /// assert_eq!(arena.remove(idx), Some(42));
    /// assert_eq!(arena.remove(idx), None);
    /// ```
    pub fn remove(&mut self, i: Index<T, I, G>) -> Option<T> {
        if i.index.to_idx() >= self.items.len() {
            return None;
        }

        let entry = mem::replace(
            &mut self.items[i.index.to_idx()],
            Entry::Free {
                next_free: self.free_list_head,
            },
        );
        match entry {
            Entry::Occupied { generation, value } => {
                if generation == i.generation {
                    self.generation.increment_generation();
                    self.free_list_head = Some(i.index);
                    self.len -= 1;
                    Some(value)
                } else {
                    self.items[i.index.to_idx()] = Entry::Occupied { generation, value };
                    None
                }
            }
            e @ Entry::Free { .. } => {
                self.items[i.index.to_idx()] = e;
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
    /// use typed_generational_arena::StandardArena;
    ///
    /// let mut crew = StandardArena::new();
    /// crew.extend(&["Jim Hawkins", "John Silver", "Alexander Smollett", "Israel Hands"]);
    /// let pirates = ["John Silver", "Israel Hands"]; // too dangerous to keep them around
    /// crew.retain(|_index, member| !pirates.contains(member));
    /// let mut crew_members = crew.iter().map(|(_, member)| **member);
    /// assert_eq!(crew_members.next(), Some("Jim Hawkins"));
    /// assert_eq!(crew_members.next(), Some("Alexander Smollett"));
    /// assert!(crew_members.next().is_none());
    /// ```
    pub fn retain(&mut self, mut predicate: impl FnMut(Index<T, I, G>, &T) -> bool) {
        for i in 0..self.items.len() {
            let remove = match &self.items[i] {
                Entry::Occupied { generation, value } => {
                    let index = Index::new(I::from_idx(i), *generation);
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
}

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> IntoIterator for Arena<T, I, G> {
    type Item = T;
    type IntoIter = IntoIter<T, I, G>;
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
/// use typed_generational_arena::StandardArena;
///
/// let mut arena = StandardArena::new();
/// for i in 0..10 {
///     arena.insert(i * i);
/// }
///
/// for value in arena {
///     assert!(value < 100);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct IntoIter<T, I: ArenaIndex, G: FixedGenerationalIndex> {
    len: usize,
    inner: vec::IntoIter<Entry<T, I, G>>,
}

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> Iterator for IntoIter<T, I, G> {
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

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> DoubleEndedIterator for IntoIter<T, I, G> {
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

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> ExactSizeIterator for IntoIter<T, I, G> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> FusedIterator for IntoIter<T, I, G> {}

impl<'a, T, I: ArenaIndex, G: FixedGenerationalIndex> IntoIterator for &'a Arena<T, I, G> {
    type Item = (Index<T, I, G>, &'a T);
    type IntoIter = Iter<'a, T, I, G>;
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
/// use typed_generational_arena::StandardArena;
///
/// let mut arena = StandardArena::new();
/// for i in 0..10 {
///     arena.insert(i * i);
/// }
///
/// for (idx, value) in &arena {
///     println!("{} is at index {:?}", value, idx);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Iter<'a, T: 'a, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> {
    len: usize,
    inner: iter::Enumerate<slice::Iter<'a, Entry<T, I, G>>>,
}

impl<'a, T, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> Iterator for Iter<'a, T, I, G> {
    type Item = (Index<T, I, G>, &'a T);

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
                    let idx = Index::new(I::from_idx(index), generation);
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

impl<'a, T, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> DoubleEndedIterator
    for Iter<'a, T, I, G>
{
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
                    let idx = Index::new(I::from_idx(index), generation);
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

impl<'a, T, I: ArenaIndex, G: FixedGenerationalIndex> ExactSizeIterator for Iter<'a, T, I, G> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T, I: ArenaIndex, G: FixedGenerationalIndex> FusedIterator for Iter<'a, T, I, G> {}

impl<'a, T, I: ArenaIndex, G: FixedGenerationalIndex> IntoIterator for &'a mut Arena<T, I, G> {
    type Item = (Index<T, I, G>, &'a mut T);
    type IntoIter = IterMut<'a, T, I, G>;
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
/// use typed_generational_arena::StandardArena;
///
/// let mut arena = StandardArena::new();
/// for i in 0..10 {
///     arena.insert(i * i);
/// }
///
/// for (_idx, value) in &mut arena {
///     *value += 5;
/// }
/// ```
#[derive(Debug)]
pub struct IterMut<'a, T: 'a, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> {
    len: usize,
    inner: iter::Enumerate<slice::IterMut<'a, Entry<T, I, G>>>,
}

impl<'a, T, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> Iterator for IterMut<'a, T, I, G> {
    type Item = (Index<T, I, G>, &'a mut T);

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
                    let idx = Index::new(I::from_idx(index), generation);
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

impl<'a, T, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> DoubleEndedIterator
    for IterMut<'a, T, I, G>
{
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
                    let idx = Index::new(I::from_idx(index), generation);
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

impl<'a, T, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> ExactSizeIterator
    for IterMut<'a, T, I, G>
{
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T, I: 'a + ArenaIndex, G: 'a + FixedGenerationalIndex> FusedIterator
    for IterMut<'a, T, I, G>
{
}

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
/// use typed_generational_arena::StandardArena;
///
/// let mut arena = StandardArena::new();
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
pub struct Drain<'a, T: 'a, I: ArenaIndex, G: FixedGenerationalIndex> {
    inner: iter::Enumerate<vec::Drain<'a, Entry<T, I, G>>>,
}

impl<'a, T, I: ArenaIndex, G: FixedGenerationalIndex> Iterator for Drain<'a, T, I, G> {
    type Item = (Index<T, I, G>, T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, Entry::Free { .. })) => continue,
                Some((index, Entry::Occupied { generation, value })) => {
                    let idx = Index::new(I::from_idx(index), generation);
                    return Some((idx, value));
                }
                None => return None,
            }
        }
    }
}

impl<T, Idx: ArenaIndex, G: FixedGenerationalIndex> Extend<T> for Arena<T, Idx, G> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<T, Idx: ArenaIndex, G: FixedGenerationalIndex> FromIterator<T> for Arena<T, Idx, G> {
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

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> ops::Index<Index<T, I, G>> for Arena<T, I, G> {
    type Output = T;

    fn index(&self, index: Index<T, I, G>) -> &Self::Output {
        self.get(index).expect("No element at index")
    }
}

impl<T, I: ArenaIndex, G: FixedGenerationalIndex> ops::IndexMut<Index<T, I, G>> for Arena<T, I, G> {
    fn index_mut(&mut self, index: Index<T, I, G>) -> &mut Self::Output {
        self.get_mut(index).expect("No element at index")
    }
}
