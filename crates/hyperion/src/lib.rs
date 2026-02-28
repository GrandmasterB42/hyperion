//! Hyperion
#![feature(read_buf)]
#![feature(core_io_borrowed_buf)]
#![feature(try_trait_v2)]
#![feature(trivial_bounds)]

use std::{fmt::Debug, net::SocketAddr, sync::Arc, time::Duration};

use bevy_app::{App, Plugin};
use bevy_ecs::{entity::Entity, event::EntityEvent, resource::Resource};
use bevy_time::{Fixed, Time};
use egress::EgressPlugin;
use hyperion_data::LocalDb;
use hyperion_net::{Compose, Global, IoBuf, Shared, lookup::LookupPlugin, proxy::init_proxy_comms};
use hyperion_proxy_proto::Crypto;
use hyperion_world::Blocks;
#[cfg(unix)]
use libc::{RLIMIT_NOFILE, getrlimit, setrlimit};
use libdeflater::CompressionLvl;
use tracing::{info, warn};
use valence_protocol::CompressionThreshold;
#[cfg(feature = "reflect")]
use {
    bevy_ecs::reflect::{ReflectEvent, ReflectResource},
    bevy_reflect::Reflect,
};

mod config;
use hyperion_crafting::CraftingRegistry;
use hyperion_utils::{
    HyperionUtilsPlugin,
    command_channel::{CommandChannel, CommandChannelPlugin},
    runtime::AsyncRuntime,
};

use crate::{
    ingress::IngressPlugin,
    simulation::{
        SimPlugin,
        skin::{ApiProvider, MojangClient, SkinHandler},
    },
    spatial::SpatialPlugin,
};

pub mod egress;
pub mod ingress;
pub mod simulation;
pub mod spatial;

// TODO: Export every crate here / Clean up some exports
// bevy_re-exports do not work properly with derive macros

// Re-exports of all internal crates
pub mod bytes {
    pub use valence_bytes::*;
}

pub mod clap {
    pub use hyperion_clap::*;
}

pub mod entity {
    pub use hyperion_entity::*;
}

pub mod gui {
    pub use hyperion_gui::*;
}

pub mod ident {
    pub use valence_ident::*;
}

pub mod inventory {
    pub use hyperion_inventory::*;
}

pub mod item {
    pub use hyperion_item::*;
}

pub mod net {
    pub use hyperion_net::*;
}

pub mod permission {
    pub use hyperion_permission::*;
}

// TODO: The valence_protocol reexports expose a lot, I need to think about if this should be slimmed down properly. Maybe just reesport the packets form hyperion_new::protocol?
pub mod protocol {
    pub use valence_protocol::*;
}

pub mod proxy {
    pub use hyperion_proxy_proto::*;
}

pub mod utils {
    pub use hyperion_utils::*;
}

pub mod world {
    pub use hyperion_world::*;
}

/// on macOS, the soft limit for the number of open file descriptors is often 256. This is far too low
/// to test 10k players with.
/// This attempts to the specified `recommended_min` value.
#[tracing::instrument(skip_all)]
#[cfg(unix)]
pub fn adjust_file_descriptor_limits(recommended_min: u64) -> std::io::Result<()> {
    use tracing::{error, warn};

    let mut limits = libc::rlimit {
        rlim_cur: 0, // Initialize soft limit to 0
        rlim_max: 0, // Initialize hard limit to 0
    };

    if unsafe { getrlimit(RLIMIT_NOFILE, &raw mut limits) } == 0 {
        // Create a stack-allocated buffer...

        info!("current soft limit: {}", limits.rlim_cur);
        info!("current hard limit: {}", limits.rlim_max);
    } else {
        error!("Failed to get the current file handle limits");
        return Err(std::io::Error::last_os_error());
    }

    if limits.rlim_max < recommended_min {
        warn!(
            "Could only set file handle limit to {}. Recommended minimum is {}",
            limits.rlim_cur, recommended_min
        );
    }

    limits.rlim_cur = limits.rlim_max;

    info!("setting soft limit to: {}", limits.rlim_cur);

    if unsafe { setrlimit(RLIMIT_NOFILE, &raw const limits) } != 0 {
        error!("Failed to set the file handle limits");
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct Endpoint(SocketAddr);

impl From<SocketAddr> for Endpoint {
    fn from(value: SocketAddr) -> Self {
        const DEFAULT_MINECRAFT_PORT: u16 = 25565;
        let port = value.port();

        if port == DEFAULT_MINECRAFT_PORT {
            warn!(
                "You are setting the port to the default Minecraft port \
                 ({DEFAULT_MINECRAFT_PORT}). You are likely using the wrong port as the proxy \
                 port is the port that players connect to. Therefore, if you want them to join on \
                 {DEFAULT_MINECRAFT_PORT}, you need to set the PROXY port to \
                 {DEFAULT_MINECRAFT_PORT} instead."
            );
        }

        Self(value)
    }
}

#[derive(EntityEvent, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Event))]
pub struct InitializePlayerPosition(pub Entity);

/// The central [`HyperionCore`] struct which owns and manages the entire server.
pub struct HyperionCore;

impl Plugin for HyperionCore {
    /// Initialize the server.
    fn build(&self, app: &mut App) {
        // 10k players * 2 file handles / player  = 20,000. We can probably get away with 16,384 file handles
        #[cfg(unix)]
        if let Err(e) = adjust_file_descriptor_limits(32_768) {
            warn!("failed to set file limits: {e}");
        }

        // Errors are ignored because they will only occur when the thread pool is initialized
        // twice, which may occur in tests that add the `HyperionCore` plugin to different apps
        let _result = rayon::ThreadPoolBuilder::new()
            .spawn_handler(|thread| {
                std::thread::Builder::new()
                    .stack_size(1024 * 1024)
                    .spawn(move || {
                        thread.run();
                    })
                    .expect("Failed to spawn thread");
                Ok(())
            })
            .build_global();

        // Initialize the compute task pool. This is done manually instead of by using
        // TaskPoolPlugin because TaskPoolPlugin also initializes AsyncComputeTaskPool and
        // IoTaskPool which are not used by Hyperion but are given 50% of the available cores.
        // Setting up ComputeTaskPool manually allows it to use 100% of the available cores.
        let mut init = false;
        bevy_tasks::ComputeTaskPool::get_or_init(|| {
            init = true;
            bevy_tasks::TaskPool::new()
        });
        if !init {
            warn!("failed to initialize ComputeTaskPool because it was already initialized");
        }

        let shared = Arc::new(Shared {
            compression_threshold: CompressionThreshold(256),
            compression_level: CompressionLvl::new(2).expect("failed to create compression level"),
        });

        info!("starting hyperion");
        let config = config::Config::load("run/config.toml").expect("failed to load config");
        app.insert_resource(config);

        let runtime = AsyncRuntime::new();

        let db = LocalDb::new().expect("failed to load database");
        let skins = SkinHandler::new(&db).expect("failed to load skin handler");

        app.insert_resource(db);
        app.insert_resource(skins);
        app.insert_resource(MojangClient::new(&runtime, ApiProvider::MAT_DOES_DEV));
        app.insert_resource(Blocks::empty(&runtime));

        let global = Global::new(shared.clone());

        app.add_plugins(CommandChannelPlugin);

        if let Some(address) = app.world().get_resource::<Endpoint>() {
            let crypto = app.world().resource::<Crypto>();
            let command_channel = app.world().resource::<CommandChannel>();
            init_proxy_comms(&runtime, command_channel.clone(), address.0, crypto.clone());
        } else {
            warn!("Endpoint was not set while loading HyperionCore");
        }

        app.insert_resource(Compose::new(
            shared.compression_level,
            global,
            IoBuf::default(),
        ));
        app.insert_resource(runtime);
        app.insert_resource(CraftingRegistry::default());

        app.add_plugins((
            bevy_time::TimePlugin,
            bevy_app::ScheduleRunnerPlugin::run_loop(Duration::from_millis(10)),
            IngressPlugin,
            EgressPlugin,
            SimPlugin,
            SpatialPlugin,
            HyperionUtilsPlugin,
            LookupPlugin,
        ));

        // Minecraft is 20 TPS
        app.insert_resource(Time::<Fixed>::from_hz(20.0));
    }
}
