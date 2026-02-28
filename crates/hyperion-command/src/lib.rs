#![cfg_attr(
    feature = "reflect",
    expect(clippy::transmute_ptr_to_ptr, clippy::used_underscore_binding)
)]
use bevy_app::{App, FixedUpdate, Plugin};

mod command_tree;
mod component;
mod system;

use bevy_ecs::schedule::IntoScheduleConfigs;
pub use command_tree::*;
pub use component::*;

pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandRegistry>().add_systems(
            FixedUpdate,
            (
                system::execute_commands,
                system::apply_deferred_changes,
                system::complete_commands,
            )
                .chain()
                .after(hyperion_net::decode::play),
        );

        let root_command = app.world_mut().spawn(Command::ROOT).id();
        app.insert_resource(RootCommand(root_command));
    }
}
