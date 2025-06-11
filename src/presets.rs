use super::{Arena, DisableRemoval, Index, NonZeroIndex, NonzeroGeneration, NonzeroWrapGeneration};

/// An arena of `T` indexed by `usize`, with `2^{64}` generations
pub type U64Arena<T> = Arena<T, usize, u64>;
/// An index into a `U64Arena`
pub type U64Index<T> = Index<T, usize, u64>;
/// A standard arena of `T` indexed by `usize`, with `2^{64} - 1` generations
pub type StandardArena<T> = Arena<T, usize, NonzeroGeneration<usize>>;
/// A typed index into a `StandardArena`
pub type StandardIndex<T> = Index<T, usize, NonzeroGeneration<usize>>;
/// An arena which can only hold up to \(2^{32} - 1\) elements and generations
pub type SmallArena<T> = Arena<T, u32, NonzeroGeneration<u32>>;
/// A typed index into a `StandardArena`
pub type SmallIndex<T> = Index<T, u32, NonzeroGeneration<u32>>;
/// An arena which can only hold up to \(2^{16}\) elements and \(2^{16} - 1\) generations
pub type TinyArena<T> = Arena<T, u16, NonzeroGeneration<u16>>;
/// A typed index into a `StandardArena`
pub type TinyIndex<T> = Index<T, u16, NonzeroGeneration<u16>>;
/// An arena which can only hold up to \(2^{16}\) elements, but unlimited
/// generations, with the caveat that generations after \(2^{16} - 1\) wrap and hence
/// may, with low probability,  collide,  leading, for example, to reading a new value
///  when the old one was deleted.
pub type TinyWrapArena<T> = Arena<T, u16, NonzeroWrapGeneration<u16>>;
/// A typed index into a `TinyWrapArena`
pub type TinyWrapIndex<T> = Index<T, u16, NonzeroWrapGeneration<u16>>;
/// An arena which can only hold up to \(2^{8}\) elements, but unlimited
/// generations, with the caveat that generations after \(2^{8}\) wrap
/// and hence may  collide, leading, for example, to reading a new value when
/// the old one was deleted.
pub type NanoArena<T> = Arena<T, u8, core::num::Wrapping<u8>>;
/// A typed index into a `NanoArena`
pub type NanoIndex<T> = Index<T, u8, core::num::Wrapping<u8>>;
/// An arena which can only hold up to \(2^{8} - 1\) elements, but unlimited
/// generations, with the caveat that generations after \(2^{8} - 1\) wrap
/// and hence may  collide, leading, for example, to reading a new value when
/// the old one was deleted.
pub type PicoArena<T> = Arena<T, u8, NonzeroWrapGeneration<u8>>;
/// A typed index into a `NanoArena`
pub type PicoIndex<T> = Index<T, u8, NonzeroWrapGeneration<u8>>;
/// A slab arena with a given index, which does *not* support efficient removal
pub type Slab<T, I> = Arena<T, I, DisableRemoval>;
/// An index into a slab of type `T` by a certain type
pub type SlabIndex<T, I> = Index<T, I, DisableRemoval>;
/// A standard slab arena which can hold up to `std::usize::MAX` elements but does
/// *not* support element removal
pub type StandardSlab<T> = Slab<T, usize>;
/// An index into a `Slab<T>`
pub type StandardSlabIndex<T> = SlabIndex<T, usize>;
/// A slab arena which can hold up to `2^{32}` elements but does *not* support
/// element removal
pub type SmallSlab<T> = Slab<T, u32>;
/// An index into a `SmallSlab<T>`
pub type SmallSlabIndex<T> = SlabIndex<T, u32>;
/// A slab arena which can hold up to `std::usize::MAX - 1` elements but does
/// *not* support element removal, and has size optimized optional indices
pub type PtrSlab<T> = Slab<T, NonZeroIndex<usize>>;
/// An index into a `PtrSlab<T>`
pub type PtrSlabIndex<T> = SlabIndex<T, NonZeroIndex<usize>>;
/// A slab arena which can hold up to `2^{32} - 1` elements but does *not* support
/// element removal, and has size optimized optional indices
pub type SmallPtrSlab<T> = Slab<T, NonZeroIndex<u32>>;
/// An index into a `SmallPtrSlab<T>`
pub type SmallPtrSlabIndex<T> = SlabIndex<T, NonZeroIndex<u32>>;
