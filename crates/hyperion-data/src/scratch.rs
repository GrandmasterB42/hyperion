use std::{alloc::Allocator, fmt::Debug};

use valence_protocol::MAX_PACKET_SIZE;

/// A scratch buffer for intermediate operations. This will return an empty [`Vec`] when calling [`Scratch::obtain`].
#[derive(Debug)]
pub struct Scratch<A: Allocator = std::alloc::Global> {
    inner: Box<[u8], A>,
}

impl Default for Scratch<std::alloc::Global> {
    fn default() -> Self {
        std::alloc::Global.into()
    }
}

/// Nice for getting a buffer that can be used for intermediate work
pub trait ScratchBuffer: sealed::Sealed + Debug {
    /// The type of the allocator the [`Vec`] uses.
    type Allocator: Allocator;
    /// Obtains a buffer that can be used for intermediate work. The contents are unspecified.
    fn obtain(&mut self) -> &mut [u8];
}

mod sealed {
    pub trait Sealed {}
}

impl<A: Allocator + Debug> sealed::Sealed for Scratch<A> {}

impl<A: Allocator + Debug> ScratchBuffer for Scratch<A> {
    type Allocator = A;

    fn obtain(&mut self) -> &mut [u8] {
        &mut self.inner
    }
}

impl<A: Allocator> From<A> for Scratch<A> {
    fn from(allocator: A) -> Self {
        // A zeroed slice is allocated to avoid reading from uninitialized memory, which is UB.
        // Allocating zeroed memory is usually very cheap, so there are minimal performance
        // penalties from this.
        let inner = Box::new_zeroed_slice_in(MAX_PACKET_SIZE as usize, allocator);
        // SAFETY: The box was initialized to zero, and u8 can be represented by zero
        let inner = unsafe { inner.assume_init() };
        Self { inner }
    }
}
