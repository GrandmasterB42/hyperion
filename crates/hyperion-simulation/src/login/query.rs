use bevy_ecs::{message::MessageReader, resource::Resource, system::Res};
use hyperion_net::{Compose, PlayerCount, packet};
use hyperion_proxy_proto::{MINECRAFT_VERSION, PROTOCOL_VERSION};
use serde_json::json;
use tracing::info;
use valence_protocol::packets::status::{QueryPongS2c, QueryResponseS2c};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectResource, bevy_reflect::Reflect};

// TODO: Is a ping response even wanted?
// This either needs to be removed, as it is not a concern of hyperion, but probably the proxy
// Or adjusted to allow for multiple different responses
#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct ServerPingResponse {
    pub description: String,
    pub max_players: u32,
}

impl Default for ServerPingResponse {
    fn default() -> Self {
        Self {
            description: String::from(
                "Getting 10k Players to PvP at Once on a Minecraft Server to Break the Guinness \
                 World Record",
            ),
            max_players: 12_000,
        }
    }
}

pub(crate) fn process_status_request(
    mut packets: MessageReader<'_, '_, packet::status::QueryRequest>,
    ping_response_data: Res<'_, ServerPingResponse>,
    compose: Res<'_, Compose>,
    player_count: Res<'_, PlayerCount>,
) {
    for packet in packets.read() {
        // let img_bytes = include_bytes!("data/hyperion.png");

        // let favicon = general_purpose::STANDARD.encode(img_bytes);
        // let favicon = format!("data:image/png;base64,{favicon}");

        // https://wiki.vg/Server_List_Ping#Response
        let json = json!({
            "version": {
                "name": MINECRAFT_VERSION,
                "protocol": PROTOCOL_VERSION,
            },
            "players": {
                "online": player_count.count,
                "max": ping_response_data.max_players,
                "sample": [],
            },
            "description": ping_response_data.description,
            // "favicon": favicon,
        });

        let json = serde_json::to_string_pretty(&json).expect("json serialization should succeed");

        let send = QueryResponseS2c {
            json: json.as_str().into(),
        };

        info!("sent query response: {packet:?}");
        compose
            .unicast_no_compression(&send, packet.connection_id())
            .unwrap();
    }
}

pub(crate) fn process_status_ping(
    mut packets: MessageReader<'_, '_, packet::status::QueryPing>,
    compose: Res<'_, Compose>,
) {
    for packet in packets.read() {
        let payload = packet.payload;
        let send = QueryPongS2c { payload };
        info!("sent ping response: {send:?}");
        compose
            .unicast_no_compression(&send, packet.connection_id())
            .unwrap();
    }
}
