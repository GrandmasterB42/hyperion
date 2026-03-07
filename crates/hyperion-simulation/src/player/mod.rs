use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{component::Component, schedule::IntoScheduleConfigs};
use glam::{DVec3, Vec3};
use hyperion_entity::{
    AiTargetable, ChunkPosition, EntityKind, EntitySize, Flight, Pitch, Velocity, Yaw,
    player::{ActiveAnimation, ConfirmBlockSequences, ImmuneStatus, Xp},
};
use hyperion_inventory::CursorItem;
use hyperion_net::decode;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

use crate::{
    player::{
        action::{creative_inventory_action, hand_swing, player_abilities, player_action},
        interact::{player_interact_block, player_interact_item},
        movement::{
            client_command, position_and_look_updates, send_pending_teleportation, update_flight,
        },
    },
    spatial::ChunkSendQueue,
};

mod action;
mod interact;
mod movement;

#[derive(Component, Debug, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
#[require(
    EntityKind::Player,
    ActiveAnimation::NONE,
    AiTargetable,
    ImmuneStatus,
    ChunkPosition::null(),
    ChunkSendQueue,
    ConfirmBlockSequences,
    Velocity,
    Pitch,
    Yaw,
    Flight,
    EntitySize,
    Xp,
    CursorItem
)]
pub struct Player;

#[derive(Component, Default, Debug, Copy, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct MovementTracking {
    pub fall_start_y: f32,
    pub last_tick_flying: bool,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    // TODO: Reflect this once glam is updated everywhere
    pub last_tick_position: Vec3,
    pub received_movement_packets: u8,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    // TODO: Reflect this once glam is updated everywhere
    pub server_velocity: DVec3,
    pub sprinting: bool,
    pub was_on_ground: bool,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MovementPlugin, InteractPlugin, ActionPlugin));
    }
}

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                (position_and_look_updates, client_command).after(decode::play),
                update_flight,
            ),
        );

        app.add_observer(send_pending_teleportation);
    }
}

pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (player_interact_item, player_interact_block).after(decode::play),
        );
    }
}

pub struct ActionPlugin;

impl Plugin for ActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                hand_swing,
                player_action,
                creative_inventory_action,
                player_abilities,
            )
                .after(hyperion_net::decode::play),
        );
    }
}
