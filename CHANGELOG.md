# 0.2.3

Released 2019-07-31

* Added `to_idx` method to `Index` to allow getting the underlying `usize` of an `Index`
* Added `IgnoredGeneration` marker trait to denote generation indices which are ignored
* Added `from_idx` method to `Index` where generations are ignored to allow turning a `usize` into an `Index`

# 0.2.2

Released 2019-06-27

* Allowed `Arena`s with fixed generations (implementing a new trait `FixedGenerationalIndex`) to be used normally as long as no elements are removed.
* Added `Slab`, `SlabIndex` and related typedefs to use new `DisableRemoval` indices to treat an `Arena` as a `Slab`

# 0.2.1

Released 2019-06-26

* Changed `StandardArena`, `SmallArena` and `TinyArena` typedefs to use new `NonzeroGeneration` generation indices for `Option<Index>` size optimization
* Changed `TinyWrapArena` typedefs to use new `NonzeroWrapGeneration` indices for `Option<Index>` size optimization
* Added `U64Arena` to replace the old `StandardArena` and `PicoArena` for a `NanoArena`-like type with `NonzeroWrapGeneration`
* Added typedefs `U64Index`, `StandardIndex`, `SmallIndex`, `TinyIndex`, `NanoIndex` and `PicoIndex` for indices into the corresponding `Arena`s

# 0.2.0

Released 2019-06-25.

* Generalized `Arena` to support arbitrary integer types for indices and generations

# 0.1.3

Released 2019-06-09.

* Added `Debug`, `Eq`, `PartialEq`, `Ord`, `PartialOrd` and `Hash` impls to `Index<T>` even when `T` did not have them
* Ensured `Index<T>` always has `Send` and `Sync` impls, even when `T` does not (without using unsafe code)
* Fixed compilation error in benches

# 0.1.2

Released 2019-06-09.

* Fixed bugs with documentation

# 0.1.1

Released 2019-06-09.

* Fixed bugs with documentation


# 0.1.0

Released 2019-06-09.

* Forked from `generational-arena` (https://github.com/fitzgen/generational-arena/)
