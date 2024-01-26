//! # Apple's C language extension of blocks
//!
//! C Blocks are the C-equivalent of Rust's closures, in that they have the
//! ability to capture their environments.
//!
//! This crate provides capabilities to create and invoke these blocks, in an
//! ergonomic "Rust-centric" fashion.
//!
//! For more information on the specifics of the block implementation, see the
//! [C language specification][lang] and the [ABI specification][ABI].
//!
//! (Note that while this library can be used separately from Objective-C,
//! they're most commonly used together).
//!
//! [lang]: https://clang.llvm.org/docs/BlockLanguageSpec.html
//! [ABI]: http://clang.llvm.org/docs/Block-ABI-Apple.html
//!
//!
//! ## Invoking blocks
//!
//! The [`Block`] struct is used for invoking blocks from Objective-C. For
//! example, consider this Objective-C function that takes a block as a
//! parameter, executes the block with some arguments, and returns the result:
//!
//! ```objc
//! #include <stdint.h>
//! #include <Block.h>
//! int32_t run_block(int32_t (^block)(int32_t, int32_t)) {
//!     return block(5, 8);
//! }
//! ```
//!
//! We could write the equivalent function in Rust like this:
//!
//! ```
//! use block2::Block;
//! unsafe fn run_block(block: &Block<(i32, i32), i32>) -> i32 {
//!     block.call((5, 8))
//! }
//! ```
//!
//! Note the extra parentheses in the `call` method, since the arguments must
//! be passed as a tuple.
//!
//!
//! ## Creating blocks
//!
//! Creating a block to pass to Objective-C can be done with [`RcBlock`] or
//! [`StackBlock`], depending on if you want to move the block to the heap,
//! or let the callee decide if it needs to do that.
//!
//! To declare external functions or methods that takes blocks, use
//! `&Block<A, R>` or `Option<&Block<A, R>>`, where `A` is a tuple with the
//! argument types, and `R` is the return type.
//!
//! As an example, we're going to work with a block that adds two integers.
//!
//! ```
//! use block2::Block;
//!
//! // External function that takes a block
//! extern "C" {
//!     fn add_numbers_using_block(block: &Block<(i32, i32), i32>);
//! }
//! #
//! # use objc2::ClassType;
//! # objc2::extern_class!(
//! #     struct MyClass;
//! #
//! #     unsafe impl ClassType for MyClass {
//! #         type Super = objc2::runtime::NSObject;
//! #         type Mutability = objc2::mutability::InteriorMutable;
//! #         const NAME: &'static str = "NSObject";
//! #     }
//! # );
//!
//! // External method that takes a block
//! objc2::extern_methods!(
//!     unsafe impl MyClass {
//!         #[method(addNumbersUsingBlock:)]
//!         pub fn addNumbersUsingBlock(&self, block: &Block<(i32, i32), i32>);
//!     }
//! );
//! ```
//!
//! To call such a function / method, we could create a new block from a
//! closure using [`RcBlock::new`].
//!
//! ```
//! use block2::RcBlock;
//! #
//! # extern "C" {
//! #     fn add_numbers_using_block(block: &block2::Block<(i32, i32), i32>);
//! # }
//! # mod imp {
//! #     #[no_mangle]
//! #     extern "C" fn add_numbers_using_block(block: &block2::Block<(i32, i32), i32>) {
//! #         assert_eq!(unsafe { block.call((5, 8)) }, 13);
//! #     }
//! # }
//!
//! let block = RcBlock::new(|a: i32, b: i32| a + b);
//! unsafe { add_numbers_using_block(&block) };
//! ```
//!
//! This creates the block on the heap. If the external function you're
//! calling is not going to copy the block, it may be more performant if you
//! construct a [`StackBlock`] directly, using [`StackBlock::new`].
//!
//! Note though that this requires that the closure is [`Clone`], as the
//! external code may want to copy the block to the heap in the future.
//!
//! ```
//! use block2::StackBlock;
//! #
//! # extern "C" {
//! #     fn add_numbers_using_block(block: &block2::Block<(i32, i32), i32>);
//! # }
//! # mod imp {
//! #     #[no_mangle]
//! #     extern "C" fn add_numbers_using_block(block: &block2::Block<(i32, i32), i32>) {
//! #         assert_eq!(unsafe { block.call((5, 8)) }, 13);
//! #     }
//! # }
//!
//! let block = StackBlock::new(|a: i32, b: i32| a + b);
//! unsafe { add_numbers_using_block(&block) };
//! ```
//!
//! As an optimization if your block doesn't capture any variables (as in the
//! above examples), you can use the [`global_block!`] macro to create a
//! static block.
//!
//! ```
//! use block2::global_block;
//! #
//! # extern "C" {
//! #     fn add_numbers_using_block(block: &block2::Block<(i32, i32), i32>);
//! # }
//! # mod imp {
//! #     #[no_mangle]
//! #     extern "C" fn add_numbers_using_block(block: &block2::Block<(i32, i32), i32>) {
//! #         assert_eq!(unsafe { block.call((5, 8)) }, 13);
//! #     }
//! # }
//!
//! global_block! {
//!     static MY_BLOCK = |a: i32, b: i32| -> i32 {
//!         a + b
//!     };
//! }
//!
//! unsafe { add_numbers_using_block(&MY_BLOCK) };
//! ```
//!
//!
//! ## Specifying a runtime
//!
//! Different runtime implementations exist and act in slightly different ways
//! (and have several different helper functions), the most important aspect
//! being that the libraries are named differently, so we must take that into
//! account when linking.
//!
//! You can choose the desired runtime by using the relevant cargo feature
//! flags, see the following sections (you might have to disable the default
//! `"apple"` feature first).
//!
//!
//! ### Apple's [`libclosure`](https://github.com/apple-oss-distributions/libclosure)
//!
//! - Feature flag: `apple`.
//!
//! This is the most common an most sophisticated runtime, and it has quite a
//! lot more features than the specification mandates.
//!
//! The minimum required operating system versions are as follows (though in
//! practice Rust itself requires higher versions than this):
//!
//! - macOS: `10.6`
//! - iOS/iPadOS: `3.2`
//! - tvOS: `1.0`
//! - watchOS: `1.0`
//!
//! **This is used by default**, so you do not need to specify a runtime if
//! you're using this crate on of these platforms.
//!
//!
//! ### LLVM `compiler-rt`'s [`libBlocksRuntime`](https://github.com/llvm/llvm-project/tree/release/13.x/compiler-rt/lib/BlocksRuntime)
//!
//! - Feature flag: `compiler-rt`.
//!
//! This is a copy of Apple's older (around macOS 10.6) runtime, and is now
//! used in [Swift's `libdispatch`] and [Swift's Foundation] as well.
//!
//! The runtime and associated headers can be installed on many Linux systems
//! with the `libblocksruntime-dev` package.
//!
//! Using this runtime probably won't work together with `objc2` crate.
//!
//! [Swift's `libdispatch`]: https://github.com/apple/swift-corelibs-libdispatch/tree/swift-5.5.1-RELEASE/src/BlocksRuntime
//! [Swift's Foundation]: https://github.com/apple/swift-corelibs-foundation/tree/swift-5.5.1-RELEASE/Sources/BlocksRuntime
//!
//!
//! ### GNUStep's [`libobjc2`](https://github.com/gnustep/libobjc2)
//!
//! - Feature flag: `gnustep-1-7`, `gnustep-1-8`, `gnustep-1-9`, `gnustep-2-0`
//!   and `gnustep-2-1` depending on the version you're using.
//!
//! GNUStep is a bit odd, because it bundles blocks support into its
//! Objective-C runtime. This means we have to link to `libobjc`, and this is
//! done by depending on the `objc2` crate. A bit unorthodox, yes, but it
//! works.
//!
//! Sources:
//!
//! - [`Block.h`](https://github.com/gnustep/libobjc2/blob/v2.1/objc/blocks_runtime.h)
//! - [`Block_private.h`](https://github.com/gnustep/libobjc2/blob/v2.1/objc/blocks_private.h)
//!
//!
//! ### Microsoft's [`WinObjC`](https://github.com/microsoft/WinObjC)
//!
//! - Feature flag: `unstable-winobjc`.
//!
//! **Unstable: Hasn't been tested on Windows yet!**
//!
//! [A fork](https://github.com/microsoft/libobjc2) based on GNUStep's `libobjc2`
//! version 1.8.
//!
//!
//! ### [`ObjFW`](https://github.com/ObjFW/ObjFW)
//!
//! - Feature flag: `unstable-objfw`.
//!
//! **Unstable: Doesn't work yet!**
//!
//!
//! ## C Compiler configuration
//!
//! To our knowledge, only Clang supports blocks. To compile C or Objective-C
//! sources using these features, you should set [the `-fblocks` flag][flag].
//!
//! [flag]: https://clang.llvm.org/docs/ClangCommandLineReference.html#cmdoption-clang-fblocks

#![no_std]
#![warn(elided_lifetimes_in_paths)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![deny(non_ascii_idents)]
#![warn(unreachable_pub)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::cargo)]
#![warn(clippy::ptr_as_ptr)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::missing_panics_doc)]
// Update in Cargo.toml as well.
#![doc(html_root_url = "https://docs.rs/block2/0.4.0")]
#![cfg_attr(feature = "unstable-docsrs", feature(doc_auto_cfg, doc_cfg_hide))]
#![cfg_attr(feature = "unstable-docsrs", doc(cfg_hide(doc)))]

extern crate alloc;
extern crate std;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
extern "C" {}

#[cfg(not(feature = "std"))]
compile_error!("The `std` feature currently must be enabled.");

#[cfg(not(any(
    feature = "apple",
    feature = "compiler-rt",
    feature = "gnustep-1-7",
    feature = "unstable-objfw",
)))]
compile_error!("A runtime must be selected");

#[cfg(any(
    all(feature = "apple", feature = "compiler-rt"),
    all(feature = "compiler-rt", feature = "gnustep-1-7"),
    all(feature = "gnustep-1-7", feature = "unstable-objfw"),
    all(feature = "unstable-objfw", feature = "apple"),
    all(feature = "apple", feature = "gnustep-1-7"),
    all(feature = "compiler-rt", feature = "unstable-objfw"),
))]
compile_error!("Only one runtime may be selected");

#[cfg(feature = "unstable-objfw")]
compile_error!("ObjFW is not yet supported");

// Link to `libclosure` (internally called `libsystem_blocks.dylib`), which is
// exported by `libSystem.dylib`.
//
// Note: Don't get confused by the presence of `System.framework`, it is a
// deprecated wrapper over the dynamic library, so we'd rather use the latter.
//
// Alternative: Only link to `libsystem_blocks.dylib`:
// println!("cargo:rustc-link-search=native=/usr/lib/system");
// println!("cargo:rustc-link-lib=dylib=system_blocks");
#[cfg_attr(feature = "apple", link(name = "System", kind = "dylib"))]
// Link to `libBlocksRuntime`.
#[cfg_attr(feature = "compiler-rt", link(name = "BlocksRuntime", kind = "dylib"))]
// Link to `libobjfw`, which provides the blocks implementation.
#[cfg_attr(feature = "unstable-objfw", link(name = "objfw", kind = "dylib"))]
extern "C" {}

// Don't link to anything on GNUStep; objc2 already does that for us!
//
// We do want to ensure linkage actually happens, though.
#[cfg(feature = "gnustep-1-7")]
extern crate objc2 as _;

mod abi;
mod block;
mod debug;
pub mod ffi;
mod global;
mod rc_block;
mod stack;
mod traits;

pub use self::block::Block;
pub use self::global::GlobalBlock;
pub use self::rc_block::RcBlock;
pub use self::stack::StackBlock;
pub use self::traits::{BlockArguments, IntoBlock};

/// Deprecated alias for `StackBlock`.
#[deprecated = "renamed to `StackBlock`"]
pub type ConcreteBlock<A, R, F> = self::stack::StackBlock<A, R, F>;

// Note: We could use `_Block_object_assign` and `_Block_object_dispose` to
// implement a `ByRef<T>` wrapper, which would behave like `__block` marked
// variables and have roughly the same memory management scheme as blocks.
//
// But I've yet to see the value in such a wrapper in Rust code compared to
// just using `Box`, `Rc` or `Arc`, and since `__block` variables are
// basically never exposed as part of a (public) function's API, we won't
// implement such a thing yet.
