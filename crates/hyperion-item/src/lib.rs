use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{
    entity::Entity,
    message::{Message, MessageReader, MessageWriter},
    schedule::IntoScheduleConfigs,
    system::Query,
};
use hyperion::{ingress, simulation::event::InteractEvent};
use hyperion_inventory::PlayerInventory;
use tracing::error;
use valence_protocol::nbt;

pub mod builder;

pub struct ItemPlugin;

/// Event sent when an item with an NBT is clicked in the hotbar
#[derive(Message)]
pub struct NbtInteractEvent {
    pub handler: Entity,
    pub event: InteractEvent,
}

impl std::ops::Deref for NbtInteractEvent {
    type Target = InteractEvent;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

fn handle_interact(
    mut events: MessageReader<'_, '_, InteractEvent>,
    query: Query<'_, '_, &PlayerInventory>,
    mut event_writer: MessageWriter<'_, NbtInteractEvent>,
) {
    for event in events.read() {
        let inventory = match query.get(event.client) {
            Ok(inventory) => inventory,
            Err(e) => {
                error!("failed to handle interact event: query failed: {e}");
                continue;
            }
        };

        let stack = &inventory.get_cursor().stack;

        if stack.is_empty() {
            return;
        }

        let Some(nbt) = stack.nbt.as_ref() else {
            return;
        };

        let Some(handler) = nbt.get("Handler") else {
            return;
        };

        let nbt::Value::Long(id) = handler else {
            return;
        };

        let id: u64 = bytemuck::cast(*id);

        let Some(handler) = Entity::try_from_bits(id) else {
            error!(
                "failed to handle interact event: nbt handler field contains an invalid entity id \
                 {id}"
            );
            return;
        };

        event_writer.write(NbtInteractEvent {
            handler,
            event: event.clone(),
        });
    }
}

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, handle_interact.after(ingress::decode::play));
        app.add_message::<NbtInteractEvent>();
    }
}
