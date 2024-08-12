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
//! use bevy::prelude::*;
//!
//! #[derive(Component, Deref, DerefMut)]
//! struct Position(Vec3);
//!
//! #[derive(Component, Deref, DerefMut)]
//! struct OldPosition(Vec3);
//!
//! // Runs in `Update` or `PostUpdate`.
//! fn interpolate_transforms(
//!     mut query: Query<(&mut Transform, &Position, &OldPosition)>,
//!     fixed_time: Res<Time<Fixed>>
//! ) {
//!     // How much of a "partial timestep" has accumulated since the last fixed timestep run.
//!     // Between `0.0` and `1.0`.
//!     let overstep_fraction = fixed_time.overstep_fraction();
//!
//!     for (mut transform, position, old_position) in &mut query {
//!         // Linearly interpolate the translation from the old position to the current one.
//!         transform.translation = old_position.lerp(position.0, overstep_fraction);
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
//! ```rust,no_run
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
//! ```rust,no_run
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
//! # use bevy::prelude::*;
//! #
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

use bevy::{
    ecs::{component::Tick, system::SystemChangeTick},
    prelude::*,
};
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

impl TransformInterpolationPlugin {
    /// Enables interpolation for translation, rotation, and scale for all entities with the [`Transform`] component.
    ///
    /// This can be overridden for individual entities by adding the [`NoTranslationInterpolation`], [`NoRotationInterpolation`],
    /// and [`NoScaleInterpolation`] components.
    pub const fn interpolate_all() -> Self {
        Self {
            global_translation_interpolation: true,
            global_rotation_interpolation: true,
            global_scale_interpolation: true,
        }
    }
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
        app.register_type::<(
            NoTranslationInterpolation,
            NoRotationInterpolation,
            NoScaleInterpolation,
        )>();

        app.init_resource::<LastEasingTick>();

        app.configure_sets(
            PostUpdate,
            TransformEasingSet.before(TransformSystem::TransformPropagate),
        );

        app.add_systems(
            FixedFirst,
            (
                reset_easing_states_on_transform_change,
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
            )
                .chain(),
        );

        app.add_systems(
            PostUpdate,
            (
                reset_easing_states_on_transform_change,
                (ease_translation, ease_rotation, ease_scale),
                update_last_easing_tick,
            )
                .chain()
                .in_set(TransformEasingSet),
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

/// A system set for transform interpolation. Runs in [`PostUpdate`], before [`TransformSystem::TransformPropagate`].
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransformEasingSet;

/// A resource that stores the last tick when easing was performed.
#[derive(Resource, Clone, Copy, Debug, Default, Deref, DerefMut)]
struct LastEasingTick(Tick);

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

fn update_last_easing_tick(
    mut last_easing_tick: ResMut<LastEasingTick>,
    system_change_tick: SystemChangeTick,
) {
    *last_easing_tick = LastEasingTick(system_change_tick.this_run());
}

/// Resets the easing states to `None` when [`Transform`] is modified outside of the fixed timestep schedules
/// or interpolation logic.
#[allow(clippy::type_complexity)]
fn reset_easing_states_on_transform_change(
    mut query: Query<
        (
            Ref<Transform>,
            Option<&mut TranslationEasingState>,
            Option<&mut RotationEasingState>,
            Option<&mut ScaleEasingState>,
        ),
        (
            Changed<Transform>,
            Or<(
                With<TranslationEasingState>,
                With<RotationEasingState>,
                With<ScaleEasingState>,
            )>,
        ),
    >,
    last_easing_tick: Res<LastEasingTick>,
    system_change_tick: SystemChangeTick,
) {
    let this_run = system_change_tick.this_run();

    for (transform, translation_easing, rotation_easing, scale_easing) in &mut query {
        let last_changed = transform.last_changed();
        let is_user_change = last_changed.is_newer_than(last_easing_tick.0, this_run);

        if !is_user_change {
            continue;
        }

        if let Some(mut translation_easing) = translation_easing {
            if translation_easing.end.is_some()
                && transform.translation != translation_easing.end.unwrap()
            {
                translation_easing.start = None;
                translation_easing.end = None;
            }
        }
        if let Some(mut rotation_easing) = rotation_easing {
            if rotation_easing.end.is_some() && transform.rotation != rotation_easing.end.unwrap() {
                rotation_easing.start = None;
                rotation_easing.end = None;
            }
        }
        if let Some(mut scale_easing) = scale_easing {
            if scale_easing.end.is_some() && transform.scale != scale_easing.end.unwrap() {
                scale_easing.start = None;
                scale_easing.end = None;
            }
        }
    }
}

/// Resets the `start` and `end` states for translation interpolation.
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

/// Resets the `start` and `end` states for rotation interpolation.
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

/// Resets the `start` and `end` states for scale interpolation.
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

/// Interpolates the translations of entities.
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

/// Interpolates the rotations of entities.
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

/// Interpolates the scales of entities.
fn ease_scale(mut query: Query<(&mut Transform, &ScaleEasingState)>, time: Res<Time<Fixed>>) {
    let overstep = time.overstep_fraction();

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.scale = start.lerp(end, overstep);
        }
    });
}
