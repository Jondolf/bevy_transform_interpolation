//! # `bevy_transform_interpolation`
//!
//! A [`Transform`] interpolation library for fixed timesteps for the [Bevy game engine](https://bevyengine.org).
//!
//! ## Features
//!
//! - Interpolate changes made to translation, rotation, and scale in [`FixedUpdate`].
//! - Interpolate individual [`Transform`] properties to reduce unnecessary computation.
//! - Apply interpolation to individual entities or to all entities.
//! - Works out of the box with physics engines using fixed timesteps.
//! - Extensible with custom easing backends.
//!
//! ## Getting Started
//!
//! First, add `bevy_transform_interpolation` as a dependency in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! bevy_transform_interpolation = { git = "https://github.com/Jondolf/bevy_transform_interpolation" }
//! ```
//!
//! To enable [`Transform`] interpolation, add the [`TransformInterpolationPlugin`] to your app:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
//!         // ...other plugins, resources, and systems
//!         .run();
//! }
//! ```
//!
//! By default, interpolation is only performed for entities with the [`TransformInterpolation`] component:
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::prelude::*;
//!
//! fn setup(mut commands: Commands) {
//!     // Interpolate the entire transform: translation, rotation, and scale.
//!     commands.spawn((
//!         Transform::default(),
//!         TransformInterpolation,
//!     ));
//! }
//! ```
//!
//! Now, any changes made to the [`Transform`] of the entity in [`FixedPreUpdate`], [`FixedUpdate`], or [`FixedPostUpdate`]
//! will automatically be interpolated in between fixed timesteps.
//!
//! If you want *all* entities with a [`Transform`] to be interpolated by default, you can use
//! [`TransformInterpolationPlugin::interpolate_all()`]:
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_transform_interpolation::prelude::*;
//!
//! fn main() {
//!    App::build()
//!       .add_plugins(TransformInterpolationPlugin::interpolate_all())
//!       // ...
//!       .run();
//! }
//! ```
//!
//! It is also possible to opt out of interpolation for individual entities, or even interpolate
//! specific [`Transform`] properties granularly. See the documentation of the [`TransformInterpolationPlugin`]
//! for more information.
//!
//! ## Custom Easing Backends
//!
//! Transforms are interpolated using easing functions, which are applied to the `start` and `end`
//! of the [`TranslationEasingState`], [`RotationEasingState`], and [`ScaleEasingState`] components.
//! These components are added and managed automatically for entities with the [`TransformInterpolation`] component.
//!
//! In the earlier example, the [`TransformInterpolationPlugin`] was used to enable interpolation.
//! However, the core easing logic and state management are actually handled by the automatically
//! added [`TransformEasingPlugin`]. The [`TransformInterpolationPlugin`] only updates the `start`
//! and `end` states of the easing.
//!
//! It is possible to replace interpolation with another approach, such as a `TransformExtrapolationPlugin`,
//! while reusing the core easing logic of the [`TransformEasingPlugin`]. An example of this can be found in
//! `examples/extrapolation.rs`.
//!
//! ## How Does It Work?
//!
//! Internally, `bevy_transform_interpolation` simply maintains components that store the `start` and `end` of the interpolation.
//! For example, translation uses the following component for easing the movement:
//!
//! ```
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
//! The actual easing is then performed in [`RunFixedMainLoop`], right after [`FixedMain`](bevy::app::FixedMain), before [`Update`].
//! If the [`Transform`] is detected to have changed since the last easing run but *outside* of the fixed timestep schedules,
//! the easing is reset to `None` to prevent overwriting the change.
//!
//! Note that the core easing logic and components are intentionally not tied to interpolation directly.
//! A physics engine could implement **transform extrapolation** using velocity and the same easing functionality,
//! supplying its own `TranslationExtrapolation` and `RotationExtrapolation` components.

#![allow(clippy::needless_doctest_main)]

pub mod background_fixed_schedule;
pub mod interpolation;

/// The prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::interpolation::*;
    #[doc(hidden)]
    pub use crate::TransformEasingPlugin;
}

// For doc links.
#[allow(unused_imports)]
use interpolation::*;

use bevy::{
    ecs::{
        component::Tick,
        schedule::{InternedScheduleLabel, InternedSystemSet, ScheduleLabel},
        system::SystemChangeTick,
    },
    prelude::*,
};

/// A plugin for applying easing to [`Transform`] changes, making movement in [`FixedUpdate`] appear smooth.
///
/// On its own, this plugin does *not* perform any automatic interpolation. It only performs easing
/// between the `start` and `end` states of the [`TranslationEasingState`], [`RotationEasingState`], and [`ScaleEasingState`]
/// components, and is responsible for resetting them at appropriate times.
///
/// To actually perform automatic easing, an easing backend that updates the `start` and `end` states must be used.
/// The [`TransformInterpolationPlugin`] is provided for transform interpolation, but custom backends can also be implemented.
#[derive(Debug)]
pub struct TransformEasingPlugin {
    pub schedule_fixed_first: InternedScheduleLabel,
    pub schedule_fixed_last: InternedScheduleLabel,
    pub schedule_fixed_loop: InternedScheduleLabel,
    pub after_fixed_main_loop: InternedSystemSet,
    /// If set to `true`, the plugin adds systems to update the easing values in [`Ease`].
    pub update_easing_values: bool,
}

impl Default for TransformEasingPlugin {
    fn default() -> Self {
        Self {
            schedule_fixed_first: FixedFirst.intern(),
            schedule_fixed_last: FixedLast.intern(),
            schedule_fixed_loop: RunFixedMainLoop.intern(),
            after_fixed_main_loop: RunFixedMainLoopSystem::AfterFixedMainLoop.intern(),
            update_easing_values: true,
        }
    }
}

impl Plugin for TransformEasingPlugin {
    fn build(&self, app: &mut App) {
        // Register easing components.
        app.register_type::<(
            TranslationEasingState,
            RotationEasingState,
            ScaleEasingState,
        )>();

        app.init_resource::<LastEasingTick>();

        // Reset easing states and update start values at the start of the fixed timestep.
        app.configure_sets(
            self.schedule_fixed_first,
            (TransformEasingSet::Reset, TransformEasingSet::UpdateStart).chain(),
        );

        // Update end values at the end of the fixed timestep.
        app.configure_sets(self.schedule_fixed_last, TransformEasingSet::UpdateEnd);

        // Perform transform easing right after the fixed timestep, before `Update`.
        app.configure_sets(
            self.schedule_fixed_loop,
            (
                TransformEasingSet::Ease,
                TransformEasingSet::UpdateEasingTick,
            )
                .chain()
                .in_set(self.after_fixed_main_loop),
        );

        // Reset easing states.
        app.add_systems(
            self.schedule_fixed_first,
            (
                reset_translation_easing,
                reset_rotation_easing,
                reset_scale_easing,
            )
                .chain()
                .in_set(TransformEasingSet::Reset),
        );

        app.add_systems(
            self.schedule_fixed_loop,
            reset_easing_states_on_transform_change.before(TransformEasingSet::Ease),
        );

        if self.update_easing_values {
            // Perform easing.
            app.add_systems(
                self.schedule_fixed_loop,
                (ease_translation_lerp, ease_rotation_slerp, ease_scale_lerp)
                    .in_set(TransformEasingSet::Ease),
            );
        }

        // Update the last easing tick.
        app.add_systems(
            self.schedule_fixed_loop,
            update_last_easing_tick.in_set(TransformEasingSet::UpdateEasingTick),
        );
    }
}

/// A system set for easing transform.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformEasingSet {
    /// Resets easing states to `None` at the start of the fixed timestep.
    Reset,
    /// Updates the `start` values for easing at the start of the fixed timestep.
    UpdateStart,
    /// Updates the `end` values for easing at the end of the fixed timestep.
    UpdateEnd,
    /// Eases the transform values in between the `start` and `end` states.
    /// Runs in [`RunFixedMainLoop`], right after [`FixedMain`](bevy::app::FixedMain), before [`Update`].
    Ease,
    /// Updates [`LastEasingTick`], the last tick when easing was performed.
    UpdateEasingTick,
}

/// A resource that stores the last tick when easing was performed.
#[derive(Resource, Clone, Copy, Debug, Default, Deref, DerefMut)]
struct LastEasingTick(Tick);

/// Stores the start and end states used for interpolating the translation of an entity.
/// The change in translation is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. Enable an easing backend
/// such as the [`TransformInterpolationPlugin`] to perform automatic interpolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct TranslationEasingState {
    /// The start translation for the interpolation.
    pub start: Option<Vec3>,
    /// The end translation for the interpolation.
    pub end: Option<Vec3>,
}

/// Stores the start and end states used for interpolating the rotation of an entity.
/// The change in rotation is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. Enable an easing backend
/// such as the [`TransformInterpolationPlugin`] to perform automatic interpolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct RotationEasingState {
    /// The start rotation for the interpolation.
    pub start: Option<Quat>,
    /// The end rotation for the interpolation.
    pub end: Option<Quat>,
}

/// Stores the start and end states used for interpolating the scale of an entity.
/// The change in scale is smoothed from `start` to `end` in between [`FixedUpdate`] runs.
///
/// On its own, this component is not updated automatically. Enable an easing backend
/// such as the [`TransformInterpolationPlugin`] to perform automatic interpolation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct ScaleEasingState {
    /// The start scale for the interpolation.
    pub start: Option<Vec3>,
    /// The end scale for the interpolation.
    pub end: Option<Vec3>,
}

fn update_last_easing_tick(
    mut last_easing_tick: ResMut<LastEasingTick>,
    system_change_tick: SystemChangeTick,
) {
    *last_easing_tick = LastEasingTick(system_change_tick.this_run());
}

/// Resets the easing states to `None` when [`Transform`] is modified outside of the fixed timestep schedules
/// or interpolation logic. This makes it possible to "teleport" entities in schedules like [`Update`].
#[allow(clippy::type_complexity, private_interfaces)]
pub fn reset_easing_states_on_transform_change(
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

    query.par_iter_mut().for_each(
        |(transform, translation_easing, rotation_easing, scale_easing)| {
            let last_changed = transform.last_changed();
            let is_user_change = last_changed.is_newer_than(last_easing_tick.0, this_run);

            if !is_user_change {
                return;
            }

            if let Some(mut translation_easing) = translation_easing {
                if let (Some(start), Some(end)) = (translation_easing.start, translation_easing.end)
                {
                    if transform.translation != start && transform.translation != end {
                        translation_easing.start = None;
                        translation_easing.end = None;
                    }
                }
            }
            if let Some(mut rotation_easing) = rotation_easing {
                if let (Some(start), Some(end)) = (rotation_easing.start, rotation_easing.end) {
                    if transform.rotation != start && transform.rotation != end {
                        rotation_easing.start = None;
                        rotation_easing.end = None;
                    }
                }
            }
            if let Some(mut scale_easing) = scale_easing {
                if let (Some(start), Some(end)) = (scale_easing.start, scale_easing.end) {
                    if transform.scale != start && transform.scale != end {
                        scale_easing.start = None;
                        scale_easing.end = None;
                    }
                }
            }
        },
    );
}

/// Resets the `start` and `end` states for translation interpolation.
fn reset_translation_easing(mut query: Query<&mut TranslationEasingState>) {
    for mut easing in &mut query {
        info!("reset_translation_easing");
        easing.start = None;
        easing.end = None;
    }
}

/// Resets the `start` and `end` states for rotation interpolation.
fn reset_rotation_easing(mut query: Query<&mut RotationEasingState>) {
    for mut easing in &mut query {
        easing.start = None;
        easing.end = None;
    }
}

/// Resets the `start` and `end` states for scale interpolation.
fn reset_scale_easing(mut query: Query<&mut ScaleEasingState>) {
    for mut easing in &mut query {
        easing.start = None;
        easing.end = None;
    }
}

/// Eases the translations of entities with linear interpolation.
fn ease_translation_lerp(
    mut query: Query<(&mut Transform, &TranslationEasingState)>,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();
    info!("ease_translation_lerp; overstep: {:?}", overstep);
    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            info!("{:?} - {:?}", start, end);
            transform.translation = start.lerp(end, overstep.min(1.0));
        }
    });
}

/// Eases the rotations of entities with spherical linear interpolation.
fn ease_rotation_slerp(
    mut query: Query<(&mut Transform, &RotationEasingState)>,
    time: Res<Time<Fixed>>,
) {
    let overstep = time.overstep_fraction();

    query
        .par_iter_mut()
        .for_each(|(mut transform, interpolation)| {
            if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
                // Note: `slerp` will always take the shortest path, but when the two rotations are more than
                // 180 degrees apart, this can cause visual artifacts as the rotation "flips" to the other side.
                transform.rotation = start.slerp(end, overstep);
            }
        });
}

/// Eases the scales of entities with linear interpolation.
fn ease_scale_lerp(mut query: Query<(&mut Transform, &ScaleEasingState)>, time: Res<Time<Fixed>>) {
    let overstep = time.overstep_fraction();

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.scale = start.lerp(end, overstep);
        }
    });
}
