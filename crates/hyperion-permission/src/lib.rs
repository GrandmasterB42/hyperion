mod storage;

use bevy_app::{App, Plugin};
use bevy_ecs::{
    component::Component,
    lifecycle::{Add, Despawn, Insert},
    observer::On,
    query::With,
    system::{Commands, Query, Res},
    world::World,
};
use clap::ValueEnum;
use hyperion::{
    net::{Compose, ConnectionId},
    simulation::{Uuid, command::get_command_packet},
    storage::LocalDb,
};
use num_derive::{FromPrimitive, ToPrimitive};
use storage::PermissionStorage;
use tracing::error;

pub struct PermissionPlugin;

#[derive(
    Default,
    Component,
    FromPrimitive,
    ToPrimitive,
    Copy,
    Clone,
    Debug,
    PartialEq,
    ValueEnum,
    Eq
)]
#[repr(C)]
pub enum Group {
    Banned,
    #[default]
    Normal,
    Moderator,
    Admin,
}

// todo:

fn load_permissions(
    new_uuid: On<'_, '_, Add, Uuid>,
    query: Query<'_, '_, &Uuid, With<ConnectionId>>,
    permissions: Res<'_, PermissionStorage>,
    mut commands: Commands<'_, '_>,
) {
    let Ok(uuid) = query.get(new_uuid.entity) else {
        return;
    };

    let group = permissions.get(**uuid);
    commands.entity(new_uuid.entity).insert(group);
}

fn store_permissions(
    group_removal: On<'_, '_, Despawn, Group>,
    query: Query<'_, '_, (&Uuid, &Group)>,
    permissions: Res<'_, PermissionStorage>,
) {
    let (uuid, group) = match query.get(group_removal.entity) {
        Ok(data) => data,
        Err(e) => {
            error!("failed to store permissions: query failed: {e}");
            return;
        }
    };

    permissions.set(**uuid, *group).unwrap();
}

fn initialize_commands(
    new_group: On<'_, '_, Insert, Group>,
    query: Query<'_, '_, &ConnectionId>,
    compose: Res<'_, Compose>,
    world: &World,
) {
    let cmd_pkt = get_command_packet(world, Some(new_group.entity));
    let Ok(&connection_id) = query.get(new_group.entity) else {
        error!("failed to initialize commands: player is missing ConnectionId");
        return;
    };
    compose.unicast(&cmd_pkt, connection_id).unwrap();
}

impl Plugin for PermissionPlugin {
    fn build(&self, app: &mut App) {
        let storage = storage::PermissionStorage::new(app.world().resource::<LocalDb>()).unwrap();
        app.insert_resource(storage);
        app.add_observer(load_permissions);
        app.add_observer(store_permissions);
        app.add_observer(initialize_commands);
    }
}
