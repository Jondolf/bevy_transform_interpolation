//! General-purpose [`Transform`] interpolation for the Bevy game engine.

mod interpolation;

use bevy::prelude::*;
pub use interpolation::*;

/// Performs transform interpolation.
#[derive(Debug, Default)]
pub struct TransformInterpolationPlugin {
    /// If `true`, translation will be interpolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoTranslationInterpolation`] component.
    pub global_translation_interpolation: bool,
    /// If `true`, rotation will be interpolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoRotationInterpolation`] component.
    pub global_rotation_interpolation: bool,
    /// If `true`, scale will be interpolated for all entities with the [`Transform`] component by default.
    ///
    /// This can be overridden for individual entities by adding the [`NoScaleInterpolation`] component.
    pub global_scale_interpolation: bool,
}

impl Plugin for TransformInterpolationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<(
            TranslationEasingState,
            RotationEasingState,
            ScaleEasingState,
        )>();
        app.register_type::<(
            TranslationInterpolation,
            RotationInterpolation,
            ScaleInterpolation,
        )>();

        app.configure_sets(
            PostUpdate,
            TransformEasingSet.before(TransformSystem::TransformPropagate),
        );

        app.add_systems(
            FixedFirst,
            (
                (
                    reset_translation_interpolation,
                    reset_rotation_interpolation,
                    reset_scale_interpolation,
                ),
                (
                    update_translation_interpolation_start,
                    update_rotation_interpolation_start,
                    update_scale_interpolation_start,
                ),
            )
                .chain(),
        );
        app.add_systems(
            FixedLast,
            (
                update_translation_interpolation_end,
                update_rotation_interpolation_end,
                update_scale_interpolation_end,
            ),
        );

        app.add_systems(
            PostUpdate,
            (ease_translation, ease_rotation, ease_scale).in_set(TransformEasingSet),
        );

        let interpolate_translation = self.global_translation_interpolation;
        let interpolate_rotation = self.global_rotation_interpolation;
        let interpolate_scale = self.global_scale_interpolation;

        app.observe(
            move |trigger: Trigger<OnAdd, Transform>, mut commands: Commands| {
                if interpolate_translation {
                    commands
                        .entity(trigger.entity())
                        .insert(TranslationInterpolation);
                }
                if interpolate_rotation {
                    commands
                        .entity(trigger.entity())
                        .insert(RotationInterpolation);
                }
                if interpolate_scale {
                    commands.entity(trigger.entity()).insert(ScaleInterpolation);
                }
            },
        );
    }
}

/// A system set for [transform interpolation]. Runs in [`PostUpdate`], before [`TransformSystem::TransformPropagate`].
///
/// [transform interpolation]: TransformInterpolation
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransformEasingSet;

/// Stores the start and end states used for interpolating the translation of an entity.
/// The change in translation is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. To perform automatic interpolation,
/// add the [`TranslationInterpolation`] component.

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct TranslationEasingState {
    pub start: Option<Vec3>,
    pub end: Option<Vec3>,
}

/// Stores the start and end states used for interpolating the rotation of an entity.
/// The change in rotation is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. To perform automatic interpolation,
/// add the [`RotationInterpolation`] component.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct RotationEasingState {
    pub start: Option<Quat>,
    pub end: Option<Quat>,
}

/// Stores the start and end states used for interpolating the scale of an entity.
/// The change in scale is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. To perform automatic interpolation,
/// add the [`ScaleInterpolation`] component.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct ScaleEasingState {
    pub start: Option<Vec3>,
    pub end: Option<Vec3>,
}

fn reset_translation_interpolation(
    mut query: Query<(&mut Transform, &mut TranslationEasingState)>,
) {
    for (mut transform, mut easing) in &mut query {
        // Make sure the previous easing is fully applied.
        if let Some(end) = easing.end {
            transform.translation = end;
        }

        easing.start = None;
        easing.end = None;
    }
}

fn reset_rotation_interpolation(mut query: Query<(&mut Transform, &mut RotationEasingState)>) {
    for (mut transform, mut easing) in &mut query {
        // Make sure the previous easing is fully applied.
        if let Some(end) = easing.end {
            transform.rotation = end;
        }

        easing.start = None;
        easing.end = None;
    }
}

fn reset_scale_interpolation(mut query: Query<(&mut Transform, &mut ScaleEasingState)>) {
    for (mut transform, mut easing) in &mut query {
        // Make sure the previous easing is fully applied.
        if let Some(end) = easing.end {
            transform.scale = end;
        }

        easing.start = None;
        easing.end = None;
    }
}

fn ease_translation(
    mut query: Query<(&mut Transform, &TranslationEasingState)>,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.translation = start.lerp(end, overstep);
        }
    });
}

fn ease_rotation(mut query: Query<(&mut Transform, &RotationEasingState)>, time: Res<Time<Fixed>>) {
    let overstep = time.overstep_fraction();

    query
        .par_iter_mut()
        .for_each(|(mut transform, interpolation)| {
            if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
                transform.rotation = start.slerp(end, overstep);
            }
        });
}

fn ease_scale(mut query: Query<(&mut Transform, &ScaleEasingState)>, time: Res<Time<Fixed>>) {
    let overstep = time.overstep_fraction();

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.scale = start.lerp(end, overstep);
        }
    });
}
