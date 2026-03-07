use bevy_ecs::component::Component;
use enumset::{EnumSet, EnumSetType};
use valence_protocol::{VarInt, packets::play::EntityAnimationS2c};
#[cfg(feature = "reflect")]
use {
    bevy_ecs::reflect::ReflectComponent,
    bevy_reflect::{Reflect, reflect_remote},
};

#[derive(EnumSetType)]
#[repr(u8)]
pub enum AnimationKind {
    SwingMainArm = 0,
    UseItem = 1,
    LeaveBed = 2,
    SwingOffHand = 3,
    Critical = 4,
    MagicCritical = 5,
}

#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct ActiveAnimation {
    #[cfg_attr(feature = "reflect", reflect(remote = RemoteEnumSetKind))]
    kind: EnumSet<AnimationKind>,
}

// TODO: This is probably a pretty bad idea, working with this internal representation
#[cfg_attr(feature = "reflect", reflect_remote(enumset::EnumSet<AnimationKind>))]
pub struct RemoteEnumSetKind {
    #[expect(clippy::pub_underscore_fields)]
    pub __priv_repr: u8,
}

impl ActiveAnimation {
    pub const NONE: Self = Self {
        kind: EnumSet::empty(),
    };

    pub fn packets(
        &mut self,
        entity_id: VarInt,
    ) -> impl Iterator<Item = EntityAnimationS2c> + use<> {
        self.kind.iter().map(move |kind| {
            let kind = kind as u8;
            EntityAnimationS2c {
                entity_id,
                animation: kind,
            }
        })
    }

    pub fn push(&mut self, kind: AnimationKind) {
        self.kind.insert(kind);
    }

    pub fn clear(&mut self) {
        self.kind.clear();
    }
}
