//! See [`AsyncRuntime`].

use std::sync::Arc;

use bevy_ecs::resource::Resource;

/// Wrapper around [`tokio::runtime::Runtime`]
#[derive(Resource, Clone)]
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
    pub(crate) fn new() -> Self {
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
