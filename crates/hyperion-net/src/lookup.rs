use std::{
    collections::{HashMap, hash_map::Entry},
    hash::BuildHasherDefault,
};

use bevy_app::Plugin;
use bevy_ecs::{
    entity::Entity,
    lifecycle::{Add, Remove},
    name::Name,
    observer::On,
    query::With,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
    world::World,
};
use hyperion_entity::{EntityKind, Uuid, player::Player};
use hyperion_proxy_proto::ConnectionId;
use rustc_hash::FxHashMap;
use tracing::{error, info};
use valence_protocol::packets::play;
use valence_text::IntoText;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectResource, bevy_reflect::Reflect};

use crate::{Compose, packet_state};

// TODO: This resource looks like it is maintained, but where could it be useful?
#[derive(Resource, Default, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct StreamLookup(
    /// The UUID of all players
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    FxHashMap<u64, Entity>,
);

impl std::ops::Deref for StreamLookup {
    type Target = FxHashMap<u64, Entity>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for StreamLookup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// TODO: This might be better of with something like a BTreeMap, maybe test performance?
// Uuids should be reasonably unique and can not directly be chosen for a DoS attack, so it's fine to use the Uuid as the hashed value
#[derive(Resource, Default, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct PlayerUuidLookup(
    /// The UUID of all players
    UuidHashMap<Entity>,
);

#[cfg_attr(feature = "reflect", derive(Reflect))]
#[derive(Default)]
pub struct UuidHasher(u64);

impl std::hash::Hasher for UuidHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // This seems to happen in chunks of 8. TODO: That might not always be the case -> this needs to be more robust
        debug_assert!(
            bytes.len() >= 8,
            "UuidHasher only supports writing in chunks of 8 bytes"
        );
        self.0 = self
            .0
            .wrapping_add(u64::from_le_bytes(bytes[0..8].try_into().unwrap()));
    }
}

type UuidHashBuilder = BuildHasherDefault<UuidHasher>;
pub type UuidHashMap<V> = HashMap<Uuid, V, UuidHashBuilder>;

impl std::ops::Deref for PlayerUuidLookup {
    type Target = UuidHashMap<Entity>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PlayerUuidLookup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Assuming the player names are reasonably unique, using the characters as the Hashmap Key should be fine?
#[derive(Resource, Debug, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct PlayerNameLookup(PlayerNameHashMap<Entity>);

#[cfg_attr(feature = "reflect", derive(Reflect))]
#[derive(Default)]
pub struct PlayerNameHasher(u64);

impl std::hash::Hasher for PlayerNameHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // TODO: This "Hashing" is pretty stupid. I probably need to benchmark this or just use another datastructure for both this and the uuidlookup
        for byte in bytes {
            self.0 = self.0.wrapping_add(u64::from(*byte) * 7_456_393);
        }
    }
}

type PlayerNameHashBuilder = BuildHasherDefault<PlayerNameHasher>;
pub type PlayerNameHashMap<V> = HashMap<String, V, PlayerNameHashBuilder>;

impl std::ops::Deref for PlayerNameLookup {
    type Target = PlayerNameHashMap<Entity>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PlayerNameLookup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct LookupPlugin;

impl Plugin for LookupPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<StreamLookup>()
            .init_resource::<PlayerUuidLookup>()
            .init_resource::<PlayerNameLookup>()
            .add_observer(initialize_player)
            .add_observer(remove_player)
            .add_observer(initialize_uuid);
    }
}

fn initialize_player(
    now_playing: On<'_, '_, Add, packet_state::Play>,
    mut name_map: ResMut<'_, PlayerNameLookup>,
    mut uuid_map: ResMut<'_, PlayerUuidLookup>,
    compose: Res<'_, Compose>,
    name_query: Query<'_, '_, (&Name, &Uuid), With<Player>>,
    connection_id_query: Query<'_, '_, &ConnectionId>,
    mut commands: Commands<'_, '_>,
) {
    // TODO: This is definitly a player required component situation, maybe on Packetstate::Play? Maybe hyperion_player crate? Maybe hyperion_entity for player and stuff like entitiykind?
    // This should really be seperate form the uuid and name lookup initialization
    commands.entity(now_playing.entity).insert((
        hyperion_entity::player::ConfirmBlockSequences::default(),
        hyperion_entity::EntitySize::default(),
        hyperion_entity::Flight::default(),
        hyperion_entity::FlyingSpeed::default(),
        hyperion_inventory::CursorItem::default(),
    ));

    let Ok((name, uuid)) = name_query.get(now_playing.entity) else {
        error!("failed to initialize player: missing Name or Uuid component");
        return;
    };

    let other_name = name_map.insert(name.to_string(), now_playing.entity);
    let other_uuid = uuid_map.insert(*uuid, now_playing.entity);

    if let Some(other) = other_name.or(other_uuid) {
        // Another player with the same username or uuid is already connected to the server.
        // Disconnect the previous player with the same username.
        // There are some Minecraft accounts with the same username, but this is an extremely
        // rare edge case which is not worth handling.

        let Ok(&other_connection_id) = connection_id_query.get(other) else {
            error!(
                "failed to kick player with same username: other player is missing ConnectionId \
                 component"
            );
            return;
        };

        let pkt = play::DisconnectS2c {
            reason: "A different player with the same username as your account has joined on a \
                     different device"
                .into_cow_text(),
        };

        compose.unicast(&pkt, other_connection_id).unwrap();
        compose.io_buf().shutdown(other_connection_id);
    }
}

fn remove_player(
    not_playing: On<'_, '_, Remove, packet_state::Play>,
    mut name_map: ResMut<'_, PlayerNameLookup>,
    mut uuid_map: ResMut<'_, PlayerUuidLookup>,
    player_query: Query<'_, '_, (&Name, &Uuid), With<Player>>,
) {
    let (name, uuid) = match player_query.get(not_playing.entity) {
        Ok(name) => name,
        Err(e) => {
            error!("failed to remove player: query failed: {e}");
            return;
        }
    };

    match name_map.entry(name.to_string()) {
        Entry::Occupied(entry) => {
            if *entry.get() == not_playing.entity {
                // This entry points to the same entity that got disconnected
                entry.remove();
            } else {
                info!(
                    "skipped removing player '{name}' from name map on disconnect: a different \
                     entity with the same name is in the name map (this could happen if the same \
                     player joined twice, causing the first player to be kicked"
                );
            }
        }
        Entry::Vacant(_) => {
            error!(
                "failed to remove player '{name}' from name map on disconnect: player is not in \
                 name map"
            );
        }
    }

    match uuid_map.entry(*uuid) {
        Entry::Occupied(entry) => {
            if *entry.get() == not_playing.entity {
                // This entry points to the same entity that got disconnected
                entry.remove();
            } else {
                info!(
                    "skipped removing player with uuid '{}' from uuid map on disconnect: a \
                     different entity with the same uuid is in the uuid map (this could happen if \
                     the same player joined twice, causing the first player to be kicked",
                    uuid.as_hyphenated()
                );
            }
        }
        Entry::Vacant(_) => {
            error!(
                "failed to remove player with uuid '{}' from uuid map on disconnect: player is \
                 not in uuid map",
                uuid.as_hyphenated()
            );
        }
    }
}

/// For every new entity without a UUID, give it one
fn initialize_uuid(known_entitykind: On<'_, '_, Add, EntityKind>, mut commands: Commands<'_, '_>) {
    let e = known_entitykind.entity;
    commands.queue(move |world: &mut World| {
        let mut entity = world.entity_mut(e);

        // This doesn't use insert_if_new to avoid the cost of generating a random uuid if it is not needed
        if entity.get::<Uuid>().is_none() {
            entity.insert(Uuid::new_v4());
        }
    });
}
