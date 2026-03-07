use std::{borrow::Cow, collections::BTreeSet};

use anyhow::Context;
use bevy_ecs::{
    entity::Entity,
    lifecycle::Add,
    message::{Message, MessageReader, MessageWriter},
    name::Name,
    observer::On,
    system::{Commands, Local, ParallelCommands, Query, Res},
    world::{FromWorld, World},
};
use colored::Colorize;
use glam::DVec3;
use hyperion_crafting::{Action, CraftingRegistry, RecipeBookState};
use hyperion_entity::{
    AiTargetable, ChunkPosition, EntityKind, PendingTeleportation, Pitch, Position, Uuid, Velocity,
    Yaw,
    player::{ActiveAnimation, ImmuneStatus, Player, Xp},
    skin::{MojangClient, PlayerSkin, SkinHandler},
};
use hyperion_net::{Compose, DataBundle, decoder::PacketDecoder, packet, packet_state};
use hyperion_proxy_proto::{Channel, ConnectionId};
use hyperion_utils::{EntityExt, command_channel::CommandChannel, runtime::AsyncRuntime};
use hyperion_world::registry::registry_codec_raw;
use tracing::{error, info, warn};
use valence_bytes::{Bytes, CowBytes, CowUtf8Bytes, Utf8Bytes};
use valence_protocol::{
    GameMode, Ident, PacketEncoder, RawBytes, VarInt,
    game_mode::OptGameMode,
    ident,
    packets::{
        login::{LoginCompressionS2c, LoginSuccessS2c},
        play::{
            self, GameJoinS2c, PlayerListS2c,
            player_list_s2c::{PlayerListActions, PlayerListEntry},
            team_s2c::{CollisionRule, Mode, NameTagVisibility, TeamColor, TeamFlags},
        },
    },
};
use valence_registry::{BiomeRegistry, RegistryCodec};
use valence_text::IntoText;

use crate::{
    config::Config,
    login::{InitializePlayerPosition, offline_uuid},
    player::MovementTracking,
    spatial::ChunkSendQueue,
};

pub(crate) fn process_login_hello(
    mut packets: MessageReader<'_, '_, packet::login::LoginHello>,
    compose: Res<'_, Compose>,
    runtime: Res<'_, AsyncRuntime>,
    skins_collection: Res<'_, SkinHandler>,
    mojang: Res<'_, MojangClient>,
    command_channel: Res<'_, CommandChannel>,
    mut commands: Commands<'_, '_>,
    mut query: Query<'_, '_, &mut PacketDecoder>,
) {
    for packet in packets.read() {
        let sender = packet.sender();
        let mut decoder = query
            .get_mut(sender)
            .expect("PacketDecoder must be available for player");

        let username = &packet.username;
        let profile_id = packet.profile_id;

        // Set compression
        let compression_threshold = compose.shared.compression_threshold;
        let pkt = LoginCompressionS2c {
            threshold: VarInt(compression_threshold.0),
        };
        compose
            .unicast_no_compression(&pkt, packet.connection_id())
            .unwrap();
        decoder.set_compression(compression_threshold);

        let uuid = profile_id.unwrap_or_else(|| offline_uuid(username));
        let uuid_s = format!("{uuid:?}").dimmed();
        info!("Starting login: {sender:?} {username} {uuid_s}");

        let pkt = LoginSuccessS2c {
            uuid,
            username: username.clone(),
            properties: Cow::default(),
        };

        compose.unicast(&pkt, packet.connection_id()).unwrap();

        let skin = if profile_id.is_some() {
            let mojang = mojang.as_ref().clone();
            let skins_collection = skins_collection.as_ref().clone();
            let command_channel = command_channel.as_ref().clone();
            runtime.spawn(async move {
                let skin = match PlayerSkin::from_uuid(uuid, &mojang, &skins_collection).await {
                    Ok(Some(skin)) => skin,
                    Err(e) => {
                        error!("failed to get skin {e}. Using empty skin");
                        PlayerSkin::EMPTY
                    }
                    Ok(None) => {
                        error!("failed to get skin. Using empty skin");
                        PlayerSkin::EMPTY
                    }
                };

                command_channel.push(move |world: &mut World| {
                    let Ok(mut entity) = world.get_entity_mut(sender) else {
                        warn!(
                            "failed to get entity after skin has been fetched (likely because the \
                             player has already left the server)"
                        );
                        return;
                    };

                    entity.insert(skin);
                });
            });
            None
        } else {
            Some(PlayerSkin::EMPTY)
        };

        let username = username.to_string();
        commands.queue(move |world: &mut World| {
            let mut entity = world.entity_mut(sender);

            // TODO: The more specific components (such as ChunkSendQueue) should be added in a
            // separate system, this might be a case for required components?
            entity.remove::<packet_state::Login>().insert((
                Player,
                Name::new(username.clone()),
                ActiveAnimation::NONE,
                AiTargetable,
                ImmuneStatus::default(),
                Uuid(uuid),
                ChunkPosition::null(),
                ChunkSendQueue::default(),
                Yaw::default(),
                Pitch::default(),
                Velocity::default(),
                Xp::default(),
                EntityKind::Player,
            ));

            world.trigger(InitializePlayerPosition(sender));

            if let Some(skin) = skin {
                let mut entity = world.entity_mut(sender);
                entity.insert(skin);
            }
        });
    }
}

#[derive(Message)]
pub struct ProcessPlayerJoin(Entity);

pub(crate) fn add_process_player_join(
    added_player_skin: On<'_, '_, Add, PlayerSkin>,
    mut events: MessageWriter<'_, ProcessPlayerJoin>,
) {
    events.write(ProcessPlayerJoin(added_player_skin.entity));
}

pub(crate) fn process_player_join(
    mut events: MessageReader<'_, '_, ProcessPlayerJoin>,
    compose: Res<'_, Compose>,
    config: Res<'_, Config>,
    target_query: Query<'_, '_, (&Uuid, &Name, &ConnectionId, &Position, &Yaw, &PlayerSkin)>,
    others_query: Query<'_, '_, (Entity, &Uuid, &Name)>,
    commands: ParallelCommands<'_, '_>,
    common_response: Local<'_, CommonPlayerJoinResponses>,
) {
    events.par_read().for_each(|event| {
        let mut bundle = DataBundle::new(&compose);

        let entity_id = event.0;
        let id = entity_id.minecraft_id();

        let (uuid, name, &connection_id, position, yaw, skin) = match target_query.get(entity_id) {
            Ok(components) => components,
            Err(e) => {
                error!("player_join_world failed: {e}");
                return;
            }
        };

        let registry_codec = registry_codec_raw();
        let codec = RegistryCodec::default();

        let dimension_names: BTreeSet<Ident> = codec
            .registry(BiomeRegistry::KEY)
            .iter()
            .map(|value| value.name.clone())
            .collect();

        let dimension_name = ident!("overworld");
        // let dimension_name: Ident<Cow<str>> = chunk_layer.dimension_type_name().into();

        let pkt = GameJoinS2c {
            entity_id: id,
            is_hardcore: false,
            dimension_names: Cow::Owned(dimension_names),
            registry_codec: Cow::Borrowed(registry_codec),
            max_players: config.max_players.into(),
            view_distance: VarInt(i32::from(config.view_distance)),
            simulation_distance: config.simulation_distance.into(),
            reduced_debug_info: false,
            enable_respawn_screen: false,
            dimension_name,
            hashed_seed: 0,
            game_mode: GameMode::Survival,
            is_flat: false,
            last_death_location: None,
            portal_cooldown: 60.into(),
            previous_game_mode: OptGameMode(Some(GameMode::Survival)),
            dimension_type_name: ident!("minecraft:overworld"),
            is_debug: false,
        };

        bundle.add_packet(&pkt).unwrap();

        let center_chunk = position.to_chunk();

        let pkt = play::ChunkRenderDistanceCenterS2c {
            chunk_x: VarInt(i32::from(center_chunk.x)),
            chunk_z: VarInt(i32::from(center_chunk.y)),
        };

        bundle.add_packet(&pkt).unwrap();

        let pkt = play::PlayerSpawnPositionS2c {
            position: position.as_dvec3().into(),
            angle: **yaw,
        };

        bundle.add_packet(&pkt).unwrap();

        bundle.add_raw(&common_response.cached_data);

        let text = play::GameMessageS2c {
            chat: format!("{name} joined the world").into_cow_text(),
            overlay: false,
        };

        compose.broadcast(&text).send().unwrap();

        // Subtracts one to exclude current player
        let others_len = others_query.iter().len() - 1;
        let mut entries = Vec::with_capacity(others_len);
        let mut all_player_names = Vec::with_capacity(others_len);

        let scope = tracing::info_span!("collect_others").entered();
        for (current_entity, uuid, name) in others_query {
            if entity_id == current_entity {
                continue;
            }

            // Update player list entries
            let entry = PlayerListEntry {
                player_uuid: uuid.0,
                username: CowUtf8Bytes::Borrowed(name),
                properties: Cow::Owned(Vec::new()),
                chat_data: None,
                listed: true,
                ping: 20,
                game_mode: GameMode::Creative,
                display_name: Some(name.to_string().into_cow_text()),
            };

            entries.push(entry);
            all_player_names.push(name.to_string());
        }
        scope.exit();

        let all_player_names = all_player_names
            .iter()
            .map(String::as_str)
            .map(Into::into)
            .collect();

        let actions = PlayerListActions::default()
            .with_add_player(true)
            .with_update_listed(true)
            .with_update_display_name(true);

        {
            let _scope = tracing::info_span!("unicasting_player_list").entered();
            bundle
                .add_packet(&PlayerListS2c {
                    actions,
                    entries: Cow::Owned(entries),
                })
                .unwrap();
        }

        let PlayerSkin {
            textures,
            signature,
        } = skin.clone();

        // todo: in future, do not clone
        let property = valence_protocol::profile::Property {
            name: Utf8Bytes::from_static("textures"),
            value: textures.into(),
            signature: Some(signature.into()),
        };

        let property = &[property];

        let singleton_entry = &[PlayerListEntry {
            player_uuid: **uuid,
            username: CowUtf8Bytes::Borrowed(name),
            properties: Cow::Borrowed(property),
            chat_data: None,
            listed: true,
            ping: 20,
            game_mode: GameMode::Survival,
            display_name: Some(name.to_string().into_cow_text()),
        }];

        let pkt = PlayerListS2c {
            actions,
            entries: Cow::Borrowed(singleton_entry),
        };

        compose.broadcast(&pkt).send().unwrap();
        bundle.add_packet(&pkt).unwrap();

        let player_name = vec![CowUtf8Bytes::Borrowed(name.as_str())];

        compose
            .broadcast(&play::TeamS2c {
                team_name: Utf8Bytes::from_static("no_tag").into(),
                mode: Mode::AddEntities {
                    entities: player_name,
                },
            })
            .exclude(connection_id)
            .send()
            .unwrap();

        bundle
            .add_packet(&play::TeamS2c {
                team_name: Utf8Bytes::from_static("no_tag").into(),
                mode: Mode::AddEntities {
                    entities: all_player_names,
                },
            })
            .unwrap();

        bundle.unicast(connection_id).unwrap();

        compose.io_buf().set_receive_broadcasts(connection_id);

        let position = **position;
        commands.command_scope(move |mut commands| {
            commands.entity(entity_id).insert((
                Channel,
                MovementTracking {
                    received_movement_packets: 0,
                    last_tick_flying: false,
                    last_tick_position: position,
                    fall_start_y: position.y,
                    server_velocity: DVec3::ZERO,
                    sprinting: false,
                    was_on_ground: false,
                },
                PendingTeleportation::new(position),
                packet_state::Play,
            ));
        });

        info!("{name} joined the world");
    });
}

pub(crate) struct CommonPlayerJoinResponses {
    pub cached_data: Bytes,
}

impl CommonPlayerJoinResponses {
    fn encode_sync_tags(encoder: &mut PacketEncoder) -> anyhow::Result<()> {
        let bytes = include_bytes!("data/tags.json");

        let groups = serde_json::from_slice(bytes)?;

        let pkt = play::SynchronizeTagsS2c { groups };

        encoder
            .append_packet(&pkt)
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }

    fn generate_cached_packet_bytes(
        encoder: &mut PacketEncoder,
        crafting_registry: &CraftingRegistry,
    ) -> anyhow::Result<()> {
        Self::encode_sync_tags(encoder)?;

        let mut buf: heapless::Vec<u8, 32> = heapless::Vec::new();
        let brand = b"hyperion";
        let brand_len = u8::try_from(brand.len()).context("brand length too long to fit in u8")?;
        buf.push(brand_len).unwrap();
        buf.extend_from_slice(brand).unwrap();

        let bytes = RawBytes::from(CowBytes::Borrowed(&buf));

        let brand = play::CustomPayloadS2c {
            channel: ident!("minecraft:brand"),
            data: bytes.into(),
        };

        encoder
            .append_packet(&brand)
            .map_err(|e| anyhow::anyhow!(e))?;

        // TODO: Is the team packet needed?
        encoder
            .append_packet(&play::TeamS2c {
                team_name: Utf8Bytes::from_static("no_tag").into(),
                mode: Mode::CreateTeam {
                    team_display_name: Cow::default(),
                    friendly_flags: TeamFlags::default(),
                    name_tag_visibility: NameTagVisibility::Never,
                    collision_rule: CollisionRule::Always,
                    team_color: TeamColor::Black,
                    team_prefix: Cow::default(),
                    team_suffix: Cow::default(),
                    entities: vec![],
                },
            })
            .map_err(|e| anyhow::anyhow!(e))?;

        if let Some(pkt) = crafting_registry.packet() {
            encoder
                .append_packet(&pkt)
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        // unlock
        let pkt = hyperion_crafting::UnlockRecipesS2c {
            action: Action::Init,
            crafting_recipe_book: RecipeBookState::FALSE,
            smelting_recipe_book: RecipeBookState::FALSE,
            blast_furnace_recipe_book: RecipeBookState::FALSE,
            smoker_recipe_book: RecipeBookState::FALSE,
            recipe_ids_1: vec!["hyperion:what".to_string()],
            recipe_ids_2: vec!["hyperion:what".to_string()],
        };

        encoder
            .append_packet(&pkt)
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}

impl FromWorld for CommonPlayerJoinResponses {
    fn from_world(world: &mut World) -> Self {
        let crafting_registry = world.get_resource::<CraftingRegistry>().unwrap();
        let compose = world.get_resource::<Compose>().unwrap();

        let compression_level = compose.shared.compression_threshold;
        let mut encoder = PacketEncoder::new();
        encoder.set_compression(compression_level);

        info!("caching world data for players with compression level {compression_level:?}");

        Self::generate_cached_packet_bytes(&mut encoder, crafting_registry).unwrap();

        Self {
            cached_data: encoder.take().freeze(),
        }
    }
}
