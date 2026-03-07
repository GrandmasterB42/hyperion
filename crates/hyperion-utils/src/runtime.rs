//! See [`AsyncRuntime`].

use std::sync::Arc;

use bevy_ecs::resource::Resource;
#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;

/// Wrapper around [`tokio::runtime::Runtime`]
#[derive(Resource, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(opaque))]
pub struct AsyncRuntime {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl std::ops::Deref for AsyncRuntime {
    type Target = tokio::runtime::Runtime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl AsyncRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::default::Default for AsyncRuntime {
    fn default() -> Self {
        Self {
            runtime: Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    // .worker_threads(2)
                    .enable_all()
                    // .thread_stack_size(1024 * 1024) // 1 MiB
                    .build()
                    .unwrap(),
            ),
        }
    }
}
