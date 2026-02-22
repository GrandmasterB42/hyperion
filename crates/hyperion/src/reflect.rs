/// Wrappers that allow reflecting foreign types
use bevy_reflect::reflect_remote;
use valence_bytes::Utf8Bytes;
use valence_ident::{Ident, ident};
use valence_protocol::{
    CompressionThreshold as Threshold,
    packets::play::command_tree_s2c::{NodeData, Parser, StringArg, Suggestion},
};
#[reflect_remote(Threshold)]
pub struct CompressionThreshold(pub i32);

#[reflect_remote(NodeData)]
pub enum NodeDataRemote {
    Root,
    Literal {
        #[reflect(ignore)]
        name: Utf8Bytes,
    },
    Argument {
        #[reflect(ignore)]
        name: Utf8Bytes,
        #[reflect(remote = ParserRemote)]
        parser: Parser,
        #[reflect(remote = OptionSuggestionRemote)]
        suggestion: Option<Suggestion>,
    },
}

// needed for remotly reflecting parser
fn default_ident() -> Ident {
    ident!("minecraft:default")
}

#[reflect_remote(Parser)]
pub enum ParserRemote {
    Bool,
    Float {
        min: Option<f32>,
        max: Option<f32>,
    },
    Double {
        min: Option<f64>,
        max: Option<f64>,
    },
    Integer {
        min: Option<i32>,
        max: Option<i32>,
    },
    Long {
        min: Option<i64>,
        max: Option<i64>,
    },
    String(#[reflect(remote=StringArgRemote)] StringArg),
    Entity {
        single: bool,
        only_players: bool,
    },
    GameProfile,
    BlockPos,
    ColumnPos,
    Vec3,
    Vec2,
    BlockState,
    BlockPredicate,
    ItemStack,
    ItemPredicate,
    Color,
    Component,
    Message,
    NbtCompoundTag,
    NbtTag,
    NbtPath,
    Objective,
    ObjectiveCriteria,
    Operation,
    Particle,
    Angle,
    Rotation,
    ScoreboardSlot,
    ScoreHolder {
        allow_multiple: bool,
    },
    Swizzle,
    Team,
    ItemSlot,
    ResourceLocation,
    Function,
    EntityAnchor,
    IntRange,
    FloatRange,
    Dimension,
    GameMode,
    Time,
    ResourceOrTag {
        #[reflect(ignore, default = "default_ident")]
        registry: Ident,
    },
    ResourceOrTagKey {
        #[reflect(ignore, default = "default_ident")]
        registry: Ident,
    },
    Resource {
        #[reflect(ignore, default = "default_ident")]
        registry: Ident,
    },
    ResourceKey {
        #[reflect(ignore, default = "default_ident")]
        registry: Ident,
    },
    TemplateMirror,
    TemplateRotation,
    Uuid,
}

#[reflect_remote(Option<Suggestion>)]
pub enum OptionSuggestionRemote {
    None,
    Some(#[reflect(remote = SuggestionRemote)] Suggestion),
}

#[reflect_remote(StringArg)]
pub enum StringArgRemote {
    SingleWord,
    QuotablePhrase,
    GreedyPhrase,
}

#[reflect_remote(Suggestion)]
pub enum SuggestionRemote {
    AskServer,
    AllRecipes,
    AvailableSounds,
    AvailableBiomes,
    SummonableEntities,
}

#[must_use]
pub fn command_permission_default()
-> fn(world: &bevy_ecs::world::World, caller: bevy_ecs::entity::Entity) -> bool {
    |_: _, _: _| true
}

// TODO: This is probably a pretty bad idea, working with this internal representation
#[reflect_remote(enumset::EnumSet<crate::simulation::animation::Kind>)]
pub struct EnumSetKindRemote {
    pub __priv_repr: u8,
}
