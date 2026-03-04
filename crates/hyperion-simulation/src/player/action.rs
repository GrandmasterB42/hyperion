use bevy_ecs::{
    message::{MessageReader, MessageWriter},
    system::{Commands, Query},
};
use glam::IVec3;
use hyperion_entity::{
    Flight,
    metadata::living_entity::HandStates,
    player::{ActiveAnimation, AnimationKind},
};
use hyperion_inventory::PlayerInventory;
use hyperion_net::packet;
use tracing::{error, warn};
use valence_protocol::{
    Hand,
    packets::play::{UpdatePlayerAbilitiesC2s, player_action_c2s::PlayerAction},
};

use crate::message;

pub(crate) fn hand_swing(
    mut packets: MessageReader<'_, '_, packet::play::HandSwing>,
    mut query: Query<'_, '_, &mut ActiveAnimation>,
) {
    for packet in packets.read() {
        let mut animation = match query.get_mut(packet.sender()) {
            Ok(animation) => animation,
            Err(e) => {
                error!("failed to handle hand swing: query failed: {e}");
                continue;
            }
        };

        match packet.hand {
            Hand::Main => {
                animation.push(AnimationKind::SwingMainArm);
            }
            Hand::Off => {
                animation.push(AnimationKind::SwingOffHand);
            }
        }
    }
}

// i.e., shooting a bow, digging a block, etc
pub(crate) fn player_action(
    mut packets: MessageReader<'_, '_, packet::play::PlayerAction>,
    mut start_destroy_writer: MessageWriter<'_, message::StartDestroyBlock>,
    mut stop_destroy_writer: MessageWriter<'_, message::DestroyBlock>,
    mut release_writer: MessageWriter<'_, message::ReleaseUseItem>,
    mut commands: Commands<'_, '_>,
) {
    for packet in packets.read() {
        let sequence = packet.sequence.0;
        let position = IVec3::new(packet.position.x, packet.position.y, packet.position.z);

        match packet.action {
            PlayerAction::StartDestroyBlock => {
                let event = message::StartDestroyBlock {
                    position,
                    from: packet.sender(),
                    sequence,
                };
                start_destroy_writer.write(event);
            }
            PlayerAction::StopDestroyBlock => {
                let event = message::DestroyBlock {
                    position,
                    from: packet.sender(),
                    sequence,
                };

                stop_destroy_writer.write(event);
            }
            PlayerAction::ReleaseUseItem => {
                let event = message::ReleaseUseItem {
                    from: packet.sender(),
                };

                commands.entity(packet.sender()).insert(HandStates::new(0));

                release_writer.write(event);
            }
            action => error!("failed to handle player action: unimplemented {action:?}"),
        }

        // todo: implement
    }
}

pub(crate) fn creative_inventory_action(
    mut packets: MessageReader<'_, '_, packet::play::CreativeInventoryAction>,
    mut query: Query<'_, '_, &mut PlayerInventory>,
) {
    for packet in packets.read() {
        // TODO: Verify that the player is in creative mode

        let Ok(slot) = u16::try_from(packet.slot) else {
            warn!("invalid slot {}", packet.slot);
            continue;
        };

        let mut inventory = match query.get_mut(packet.sender()) {
            Ok(inventory) => inventory,
            Err(e) => {
                error!("failed to handle creative inventory action: query failed: {e}");
                continue;
            }
        };

        if let Err(e) = inventory.set(slot, packet.clicked_item.clone()) {
            error!("failed to handle creative inventory action: inventory set failed: {e}");
        }
    }
}

pub(crate) fn player_abilities(
    mut packets: MessageReader<'_, '_, packet::play::UpdatePlayerAbilities>,
    mut query: Query<'_, '_, &mut Flight>,
) {
    for packet in packets.read() {
        let mut flight = match query.get_mut(packet.sender()) {
            Ok(flight) => flight,
            Err(e) => {
                error!("player abilities failed: query failed: {e}");
                continue;
            }
        };

        match **packet {
            UpdatePlayerAbilitiesC2s::StopFlying => flight.is_flying = false,
            UpdatePlayerAbilitiesC2s::StartFlying => flight.is_flying = flight.allow,
        }
    }
}
