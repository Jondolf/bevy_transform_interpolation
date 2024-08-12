//! # `bevy_transform_interpolation`
//!
//! A general-purpose [`Transform`] interpolation plugin for fixed timesteps for the [Bevy game engine](https://bevyengine.org).
//!
//! ## What is this for?
//!
//! A lot of gameplay logic and movement systems typically use a fixed timestep to produce consistent and stable behavior
//! regardless of the frame rate. Notable examples include physics simulation and character movement.
//!
//! However, this can make movement appear choppy, especially on displays with a high refresh rate.
//! To achieve visually smooth movement while using a fixed timestep, the visual transform must be smoothed
//! independently of the "true" gameplay transform.
//!
//! The most common way to do this is to use **transform interpolation**, which interpolates movement from the previous
//! state to the current state. This could be done by storing the current and old gameplay positions in their own components
//! and interpolating [`Transform`] using them:
//!
//! ```rust
//! #[derive(Component)]
//! struct Position(Vec3);
//!
//! #[derive(Component)]
//! struct PreviousPosition(Vec3);
//!
//! // Runs in `Update` or `PostUpdate`.
//! fn interpolate_transforms(
//!     query: Query<(&mut Transform, &Position, &OldPosition)>,
//!     fixed_time: Res<Time<Fixed>>
//! ) {
//!     // How much of a "partial timestep" has accumulated since the last fixed timestep run.
//!     // Between `0.0` and `1.0`.
//!     let overstep = fixed_time.overstep_fraction();
//!
//!     for (mut transform, position, old_position) in &mut query {
//!         // Linearly interpolate the translation from the old position to the current one.
//!         transform.translation = old_position.lerp(position, overstep_fraction);
//!     }
//! }
//! ```
//!
//! In fact, you could simply plug the above implementation into your own application if you wanted to!
//!
//! However, it requires you to use `Position` for gameplay logic, and to manage `OldPosition` somewhere.
//! This can be annoying, and is incompatibile with third party libraries that expect to be able to modify
//! the transform directly.
//!
//! `bevy_transform_interpolation` aims to be a drop-in solution that allows easy and efficient transform interpolation,
//! while still allowing the usage of [`Transform`] for gameplay logic. It should be automatically compatible with physics engines
//! such as [Avian](https://github.com/Jondolf/avian) and [`bevy_rapier`](https://github.com/dimforge/bevy_rapier), as long as
//! the simulation is run in [`FixedUpdate`] or [`FixedPostUpdate`].
//!
//! ## Usage
//!
//! First, add `bevy_transform_interpolation` to your dependencies in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! bevy_transform_interpolation = { git = "https://github.com/Jondolf/bevy_transform_interpolation" }
//! ```
//!
//! Next, add the [`TransformInterpolationPlugin`]:
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
//!         // ...other plugins, resources, and systems
//!         .run();
//! }
//! ```
//!
//! Transform interpolation can be enabled very granularly in `bevy_transform_interpolation`.
//! You can choose to interpolate transform, rotation, or scale individually, or use any combination of them:
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::*;
//!
//! fn setup(mut commands: Commands) {
//!     // Only interpolate translation.
//!     commands.spawn((TransformBundle::default(), TranslationInterpolation));
//!     
//!     // Only interpolate rotation.
//!     commands.spawn((TransformBundle::default(), RotationInterpolation));
//!     
//!     // Only interpolate scale.
//!     commands.spawn((TransformBundle::default(), ScaleInterpolation));
//!     
//!     // Interpolate translation and rotation, but not scale.
//!     commands.spawn((
//!         TransformBundle::default(),
//!         TranslationInterpolation,
//!         RotationInterpolation,
//!     ));
//!     
//!     // Interpolate the entire transform: translation, rotation, and scale.
//!     // The components can be added individually, or using the `TransformInterpolationBundle`.
//!     commands.spawn((
//!         TransformBundle::default(),
//!         TransformInterpolationBundle::default(),
//!     ));
//! }
//! ```
//!
//! You can also enable transform interpolation globally for *all* entities that have a [`Transform`]
//! by configuring the [`TransformInterpolationPlugin`]:
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((
//!             DefaultPlugins,
//!             // Interpolate translation and rotation, but not scale.
//!             TransformInterpolationPlugin {
//!                 global_translation_interpolation: true,
//!                 global_rotation_interpolation: true,
//!                 global_scale_interpolation: false,
//!             },
//!         ))
//!         // ...other plugins, resources, and systems
//!         .run();
//! }
//! ```
//!
//! If interpolation is enabled globally, it can still be disabled for individual entities using the [`NoTranslationInterpolation`],
//! [`NoRotationInterpolation`], and [`NoScaleInterpolation`] components.
//!
//! Now, any changes made to [`Transform`] in [`FixedPreUpdate`], [`FixedUpdate`], or [`FixedPostUpdate`] will automatically
//! be smoothed in between the fixed timesteps for entities that have transform interpolation enabled.
//!
//! Changing [`Transform`] manually in any schedule that *doesn't* use a fixed timestep is also supported,
//! but it is equivalent to teleporting, and disables interpolation for the entity for the remainder of that fixed timestep.
//!
//! ## How Does It Work?
//!
//! Internally, `bevy_transform_interpolation` simply maintains components that store the `start` and `end` of the interpolation.
//! For example, translation uses the following component for easing the movement:
//!
//! ```rust
//! pub struct TranslationEasingState {
//!     pub start: Option<Vec3>,
//!     pub end: Option<Vec3>,
//! }
//! ```
//!
//! - At the start of the [`FixedFirst`] schedule, the states are reset to `None`.
//! - In [`FixedFirst`], for every entity with the [`TranslationInterpolation`] component, `start` is set to the current [`Transform`].
//! - In [`FixedLast`], for every entity with the [`TranslationInterpolation`] component, `end` is set to the current [`Transform`].
//!
//! This way, `start` represents the "old" state, while `end` represents the "new" state after changes have been made to [`Transform`]
//! in between [`FixedFirst`] and [`FixedLast`]. Rotation and scale are handled similarly.
//!
//! The easing is then performed in [`PostUpdate`], before Bevy's transform propagation systems. If the [`Transform`] is detected to have changed
//! since the last easing run but *outside* of the fixed timestep schedules, the easing is reset to `None` to prevent overwriting the change.
//!
//! Note that the core easing logic and components are intentionally not tied to interpolation directly.
//! A physics engine could implement **transform extrapolation** using velocity and the same easing functionality,
//! supplying its own `TranslationExtrapolation` and `RotationExtrapolation` components.
//!
//! ## Caveats
//!
//! - In cases where the previous or current gameplay transform are already stored separately from [`Transform`],
//!   storing them in the easing states as well may be redundant. Although it *is* still useful for allowing
//!   [`Transform`] to be modified directly and for wider compatibility with the ecosystem.
//! - Transform extrapolation is currently not supported as a built-in feature, as it typically requires a velocity
//!   for the prediction of the next state. However, it could be supported by external libraries such as physics engines
//!   in a similar way to `src/interpolation.rs`, and simply updating the `start` and `end` states differently.

#![allow(clippy::needless_doctest_main)]

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
