use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{
    component::Component,
    lifecycle::Add,
    message::MessageReader,
    name::Name,
    observer::On,
    schedule::IntoScheduleConfigs,
    system::{Commands, Query, Res},
};
use hyperion::{
    ingress,
    net::{Compose, ConnectionId},
    simulation::{Position, packet, packet_state},
};
use tracing::error;
use valence_protocol::{
    packets::play,
    text::{Color, IntoText, Text},
};

use crate::Team;

const CHAT_COOLDOWN_SECONDS: i64 = 3; // 3 seconds
const CHAT_COOLDOWN_TICKS: i64 = CHAT_COOLDOWN_SECONDS * 20; // Convert seconds to ticks

#[derive(Default, Component)]
pub struct ChatCooldown {
    pub expires: i64,
}

pub fn initialize_cooldown(
    now_playing: On<'_, '_, Add, packet_state::Play>,
    mut commands: Commands<'_, '_>,
) {
    commands
        .entity(now_playing.entity)
        .insert(ChatCooldown::default());
}

pub fn handle_chat_messages(
    mut packets: MessageReader<'_, '_, packet::play::ChatMessage>,
    compose: Res<'_, Compose>,
    mut query: Query<'_, '_, (&Name, &Position, &mut ChatCooldown, &ConnectionId, &Team)>,
) {
    let current_tick = compose.global().tick;

    for packet in packets.read() {
        let (name, position, mut cooldown, io, team) = match query.get_mut(packet.sender()) {
            Ok(data) => data,
            Err(e) => {
                error!("could not process chat message: query failed: {e}");
                continue;
            }
        };

        // Check if player is still on cooldown
        if cooldown.expires > current_tick {
            let remaining_ticks = cooldown.expires - current_tick;
            #[expect(clippy::cast_precision_loss)]
            let remaining_secs = remaining_ticks as f32 / 20.0;

            let cooldown_msg =
                format!("§cPlease wait {remaining_secs:.2} seconds before sending another message")
                    .into_cow_text();

            let packet = play::GameMessageS2c {
                chat: cooldown_msg,
                overlay: false,
            };

            compose.unicast(&packet, *io).unwrap();
            continue;
        }

        cooldown.expires = current_tick + CHAT_COOLDOWN_TICKS;

        let chat = Text::default()
            + "<".color(Color::DARK_GRAY)
            + name.as_str().to_owned().color(*team)
            + "> ".color(Color::DARK_GRAY)
            + (**packet.message).to_owned();
        let packet = play::GameMessageS2c {
            chat: chat.into(),
            overlay: false,
        };

        let center = position.to_chunk();

        compose.broadcast_local(&packet, center).send().unwrap();
    }
}

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(initialize_cooldown);
        app.add_systems(
            FixedUpdate,
            handle_chat_messages.after(ingress::decode::play),
        );
    }
}
