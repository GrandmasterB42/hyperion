//! Constructs for connecting and working with a `Heed` database.

use std::path::Path;

use bevy_ecs::resource::Resource;
#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;
use heed::{Env, EnvOpenOptions};

/// A wrapper around a `Heed` database
#[derive(Resource, Debug, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(opaque))]
pub struct LocalDb {
    pub env: Env,
}

impl LocalDb {
    /// Creates a new [`LocalDb`]
    pub fn new() -> anyhow::Result<Self> {
        let path = Path::new("db").join("heed.mdb");

        std::fs::create_dir_all(&path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(10 * 1024 * 1024) // 10MB
                .max_dbs(8) // todo: why is this needed/configurable? ideally would be infinite...
                .open(&path)?
        };

        Ok(Self { env })
    }
}

impl std::ops::Deref for LocalDb {
    type Target = Env;

    fn deref(&self) -> &Self::Target {
        &self.env
    }
}
