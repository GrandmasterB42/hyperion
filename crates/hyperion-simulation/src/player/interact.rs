use bevy_ecs::{
    message::{MessageReader, MessageWriter},
    system::{Query, Res},
};
use geometry::aabb::Aabb;
use glam::IVec3;
use hyperion_entity::{EntitySize, Position, player::ConfirmBlockSequences};
use hyperion_inventory::PlayerInventory;
use hyperion_item::ItemInteractEvent;
use hyperion_net::{Compose, packet};
use hyperion_world::Blocks;
use tracing::{error, warn};
use valence_protocol::{
    BlockKind, BlockState, ItemKind, block::PropName, packets::play::OpenWrittenBookS2c,
};

use crate::message;

/// Handles player interaction with items in hand
///
/// Common uses:
/// - Starting to wind up a bow for shooting arrows
/// - Using consumable items like food or potions
/// - Throwing items like snowballs or ender pearls
/// - Using tools/items with special right-click actions (e.g. fishing rods, shields)
/// - Activating items with duration effects (e.g. chorus fruit teleport)
pub(crate) fn player_interact_item(
    mut packets: MessageReader<'_, '_, packet::play::PlayerInteractItem>,
    compose: Res<'_, Compose>,
    query: Query<'_, '_, &PlayerInventory>,
    mut item_interact_writer: MessageWriter<'_, ItemInteractEvent>,
) {
    for packet in packets.read() {
        let inventory = match query.get(packet.sender()) {
            Ok(inventory) => inventory,
            Err(e) => {
                error!("failed to process player interact item: query failed: {e}");
                continue;
            }
        };

        let event = ItemInteractEvent {
            entity: packet.sender(),
            hand: packet.hand,
            sequence: packet.sequence.0,
        };

        let cursor = &inventory.get_cursor().stack;

        if !cursor.is_empty() && cursor.item == ItemKind::WrittenBook {
            compose
                .unicast(
                    &OpenWrittenBookS2c { hand: packet.hand },
                    packet.connection_id(),
                )
                .unwrap();
        }

        item_interact_writer.write(event);
    }
}

pub(crate) fn player_interact_block(
    mut packets: MessageReader<'_, '_, packet::play::PlayerInteractBlock>,
    mut query: Query<
        '_,
        '_,
        (
            &mut ConfirmBlockSequences,
            &PlayerInventory,
            &Position,
            &EntitySize,
        ),
    >,
    blocks: Res<'_, Blocks>,
    mut toggle_door_writer: MessageWriter<'_, message::ToggleDoor>,
    mut place_block_writer: MessageWriter<'_, message::PlaceBlock>,
) {
    for packet in packets.read() {
        // PlayerInteractBlock contains:
        // - hand: Hand (enum: MainHand or OffHand)
        // - position: BlockPos (x, y, z coordinates of the block)
        // - face: Direction (enum: Down, Up, North, South, West, East)
        // - cursor_position: Vec3 (x, y, z coordinates of cursor on the block face)
        // - inside_block: bool (whether the player's head is inside a block)
        // - sequence: VarInt (sequence number for this interaction)

        let (mut confirm_block_sequences, inventory, client_position, size) =
            match query.get_mut(packet.sender()) {
                Ok(data) => data,
                Err(e) => {
                    error!("failed to handle player interact block: query failed: {e}");
                    continue;
                }
            };

        confirm_block_sequences.push(packet.sequence.0);

        let interacted_block_pos = packet.position;
        let interacted_block_pos_vec = IVec3::new(
            interacted_block_pos.x,
            interacted_block_pos.y,
            interacted_block_pos.z,
        );

        let Some(interacted_block) = blocks.get_block(interacted_block_pos_vec) else {
            continue;
        };

        if interacted_block.get(PropName::Open).is_some() {
            // Toggle the open state of a door
            // todo: place block instead of toggling door if the player is crouching and holding a
            // block

            toggle_door_writer.write(message::ToggleDoor {
                position: interacted_block_pos_vec,
                from: packet.sender(),
                sequence: packet.sequence.0,
            });
        } else {
            // Attempt to place a block

            let held = &inventory.get_cursor().stack;

            if held.is_empty() {
                continue;
            }

            let kind = held.item;

            let Some(block_kind) = BlockKind::from_item_kind(kind) else {
                warn!("invalid item kind to place: {kind:?}");
                continue;
            };

            let block_state = BlockState::from_kind(block_kind);

            let position = interacted_block_pos.get_in_direction(packet.face);
            let position = IVec3::new(position.x, position.y, position.z);

            let position_dvec3 = position.as_vec3();

            // todo(hack): technically players can do some crazy position stuff to abuse this probably
            let player_aabb = size.aabb(**client_position);

            let collides_player = block_state
                .collision_shapes()
                .map(|aabb| {
                    Aabb::new(aabb.min().as_vec3(), aabb.max().as_vec3()).move_by(position_dvec3)
                })
                .any(|block_aabb| Aabb::overlap(&block_aabb, &player_aabb).is_some());

            if collides_player {
                continue;
            }

            place_block_writer.write(message::PlaceBlock {
                position,
                from: packet.sender(),
                sequence: packet.sequence.0,
                block: block_state,
            });
        }
    }
}
