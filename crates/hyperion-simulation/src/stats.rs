use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{
    lifecycle::{Add, Remove},
    observer::On,
    system::ResMut,
};
use hyperion_net::{PlayerCount, TickData, packet_state};
use hyperion_world::Blocks;

pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (global_update, load_pending)); // TODO: This should definitely happen before FixedUpdate, right? 
        app.add_observer(player_join_world);
        app.add_observer(player_leave_world);
    }
}

fn global_update(mut tick: ResMut<'_, TickData>) {
    tick.tick += 1;
}

pub fn player_join_world(
    _: On<'_, '_, Add, packet_state::Play>,
    mut player_count: ResMut<'_, PlayerCount>,
) {
    player_count.count += 1;
}

pub fn player_leave_world(
    _: On<'_, '_, Remove, packet_state::Play>,
    mut player_count: ResMut<'_, PlayerCount>,
) {
    player_count.count -= 1;
}

fn load_pending(mut blocks: ResMut<'_, Blocks>) {
    blocks.load_pending();
}
