use bevy_app::{App, Plugin};
use hyperion_entity::metadata::MetadataPlugin;
use hyperion_net::proxy::RequestSubscribeChannelPackets;

use crate::{
    channel::ChannelPlugin, inventory::InventoryPlugin, login::PlayerJoinPlugin,
    player::PlayerPlugin, spatial::SyncChunksPlugin, stats::StatsPlugin, sync::SyncPlugin,
};

pub mod channel;
pub mod config;
pub mod inventory;
pub mod login;
pub mod message;
pub mod player;
pub mod spatial;
pub mod stats;
pub mod sync;

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SyncPlugin,
            PlayerPlugin,
            ChannelPlugin,
            InventoryPlugin,
            MetadataPlugin,
            StatsPlugin,
            SyncChunksPlugin,
            PlayerJoinPlugin,
        ));

        app.add_message::<RequestSubscribeChannelPackets>();
        app.add_message::<message::ItemDropEvent>();
        app.add_message::<message::SetSkin>();
        app.add_message::<message::AttackEntity>();
        app.add_message::<message::StartDestroyBlock>();
        app.add_message::<message::DestroyBlock>();
        app.add_message::<message::PlaceBlock>();
        app.add_message::<message::ToggleDoor>();
        app.add_message::<message::SwingArm>();
        app.add_message::<message::ReleaseUseItem>();
        app.add_message::<message::PostureUpdate>();
        app.add_message::<message::BlockInteract>();
        app.add_message::<message::ProjectileEntityEvent>();
        app.add_message::<message::ProjectileBlockEvent>();
        app.add_message::<message::ClickSlotEvent>();
        app.add_message::<message::DropItemStackEvent>();
        app.add_message::<message::UpdateSelectedSlotEvent>();
        app.add_message::<message::HitGroundEvent>();
    }
}
