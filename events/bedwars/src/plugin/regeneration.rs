use bevy_app::{App, FixedPostUpdate, Plugin};
use bevy_ecs::{
    component::Component,
    lifecycle::Add,
    observer::On,
    system::{Commands, Query, Res},
};
use hyperion::{
    net::Compose,
    simulation::{metadata::living_entity::Health, packet_state},
};
use hyperion_utils::Prev;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

const MAX_HEALTH: f32 = 20.0;

pub struct RegenerationPlugin;

#[derive(Component, Default, Copy, Clone, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct LastDamaged {
    pub tick: i64,
}

fn initialize_player(
    now_playing: On<'_, '_, Add, packet_state::Play>,
    mut commands: Commands<'_, '_>,
) {
    commands
        .entity(now_playing.entity)
        .insert(LastDamaged::default());
}

fn regenerate(
    query: Query<'_, '_, (&mut LastDamaged, &Prev<Health>, &mut Health)>,
    compose: Res<'_, Compose>,
) {
    let current_tick = compose.global().tick;

    for (mut last_damaged, prev_health, mut health) in query {
        if *health < **prev_health {
            last_damaged.tick = current_tick;
        }

        let ticks_since_damage = current_tick - last_damaged.tick;

        if health.is_dead() {
            return;
        }

        // Calculate regeneration rate based on time since last damage
        let base_regen = 0.01; // Base regeneration per tick
        let ramp_factor = 0.0001_f32; // Increase in regeneration per tick
        let max_regen = 0.1; // Maximum regeneration per tick

        #[expect(clippy::cast_precision_loss)]
        let regen_rate = ramp_factor
            .mul_add(ticks_since_damage as f32, base_regen)
            .min(max_regen);

        // Apply regeneration, capped at max health
        health.heal(regen_rate);
        **health = health.min(MAX_HEALTH);
    }
}

impl Plugin for RegenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(initialize_player);
        app.add_systems(FixedPostUpdate, regenerate);
    }
}
