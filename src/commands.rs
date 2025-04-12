//! Helper commands for operations on interpolated or extrapolated entities.

use bevy::{
    ecs::{entity::Entity, system::Command, world::World},
    reflect::prelude::*,
};

use crate::{RotationEasingState, ScaleEasingState, TranslationEasingState};

/// A [`Command`] that resets the easing states of an entity.
///
/// This disables easing for the remainder of the current fixed time step,
/// allowing you to freely set the [`Transform`](bevy::transform::components::Transform)
/// of the entity without any easing being applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[reflect(Debug, PartialEq)]
pub struct ResetEasing(pub Entity);

impl Command for ResetEasing {
    fn apply(self, world: &mut World) {
        let Ok(mut entity_mut) = world.get_entity_mut(self.0) else {
            return;
        };

        if let Some(mut translation_easing) = entity_mut.get_mut::<TranslationEasingState>() {
            translation_easing.start = None;
            translation_easing.end = None;
        }

        if let Some(mut rotation_easing) = entity_mut.get_mut::<RotationEasingState>() {
            rotation_easing.start = None;
            rotation_easing.end = None;
        }

        if let Some(mut scale_easing) = entity_mut.get_mut::<ScaleEasingState>() {
            scale_easing.start = None;
            scale_easing.end = None;
        }
    }
}
