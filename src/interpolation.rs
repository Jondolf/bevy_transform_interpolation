#![allow(clippy::type_complexity)]

use crate::*;
use bevy::{
    ecs::component::{ComponentHooks, StorageType},
    prelude::*,
};

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
            TranslationInterpolation,
            RotationInterpolation,
            ScaleInterpolation,
        )>();
        app.register_type::<(
            NoTranslationInterpolation,
            NoRotationInterpolation,
            NoScaleInterpolation,
        )>();

        app.add_systems(
            FixedFirst,
            (
                update_translation_interpolation_start,
                update_rotation_interpolation_start,
                update_scale_interpolation_start,
            )
                .chain()
                .in_set(TransformEasingSet::UpdateStart),
        );
        app.add_systems(
            FixedLast,
            (
                update_translation_interpolation_end,
                update_rotation_interpolation_end,
                update_scale_interpolation_end,
            )
                .chain()
                .in_set(TransformEasingSet::UpdateEnd),
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

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<TransformEasingPlugin>() {
            app.add_plugins(TransformEasingPlugin);
        }
    }
}

/// A bundle for enabling full [transform interpolation] for an entity.
/// Changes in translation, rotation, and scale are smoothed between [`FixedUpdate`] runs.
///
/// [transform interpolation]: crate
#[derive(Bundle, Clone, Copy, Debug, Default, PartialEq)]
pub struct TransformInterpolationBundle {
    pub translation_interpolation: TranslationInterpolation,
    pub translation_easing: TranslationEasingState,
    pub rotation_interpolation: RotationInterpolation,
    pub rotation_easing: RotationEasingState,
    pub scale_interpolation: ScaleInterpolation,
    pub scale_easing: ScaleEasingState,
}

/// Enables automatic translation interpolation for an entity.
///
/// Changes in translation are smoothed between [`FixedUpdate`] runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct TranslationInterpolation;

/// Enables automatic rotation interpolation for an entity.
///
/// Changes in rotation are smoothed between [`FixedUpdate`] runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct RotationInterpolation;

/// Enables automatic scale interpolation for an entity.
///
/// Changes in scale are smoothed between [`FixedUpdate`] runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct ScaleInterpolation;

impl Component for TranslationInterpolation {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_add(|mut world, entity, _| {
            if let Some(mut easing) = world.get_mut::<TranslationEasingState>(entity) {
                *easing = TranslationEasingState::default();
            } else {
                world
                    .commands()
                    .entity(entity)
                    .insert(TranslationEasingState::default());
            }
        });
    }
}

impl Component for RotationInterpolation {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_add(|mut world, entity, _| {
            if let Some(mut easing) = world.get_mut::<RotationEasingState>(entity) {
                *easing = RotationEasingState::default();
            } else {
                world
                    .commands()
                    .entity(entity)
                    .insert(RotationEasingState::default());
            }
        });
    }
}

impl Component for ScaleInterpolation {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_add(|mut world, entity, _| {
            if let Some(mut easing) = world.get_mut::<ScaleEasingState>(entity) {
                *easing = ScaleEasingState::default();
            } else {
                world
                    .commands()
                    .entity(entity)
                    .insert(ScaleEasingState::default());
            }
        });
    }
}

/// Explicitly marks this entity as having translation interpolation disabled.
///
/// This can be used to override [`TransformInterpolationPlugin::global_translation_interpolation`]
/// for this entity if the option is `true`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NoTranslationInterpolation;

/// Explicitly marks this entity as having rotation interpolation disabled.
///
/// This can be used to override [`TransformInterpolationPlugin::global_rotation_interpolation`]
/// for this entity if the option is `true`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NoRotationInterpolation;

/// Explicitly marks this entity as having scale interpolation disabled.
///
/// This can be used to override [`TransformInterpolationPlugin::global_scale_interpolation`]
/// for this entity if the option is `true`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
pub struct NoScaleInterpolation;

pub(crate) fn update_translation_interpolation_start(
    mut query: Query<
        (&Transform, &mut TranslationEasingState),
        (
            With<TranslationInterpolation>,
            Without<NoTranslationInterpolation>,
        ),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.start = Some(transform.translation);
    }
}

pub(crate) fn update_translation_interpolation_end(
    mut query: Query<
        (&Transform, &mut TranslationEasingState),
        (
            With<TranslationInterpolation>,
            Without<NoTranslationInterpolation>,
        ),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.end = Some(transform.translation);
    }
}

pub(crate) fn update_rotation_interpolation_start(
    mut query: Query<
        (&Transform, &mut RotationEasingState),
        (
            With<RotationInterpolation>,
            Without<NoRotationInterpolation>,
        ),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.start = Some(transform.rotation);
    }
}

pub(crate) fn update_rotation_interpolation_end(
    mut query: Query<
        (&Transform, &mut RotationEasingState),
        (
            With<RotationInterpolation>,
            Without<NoRotationInterpolation>,
        ),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.end = Some(transform.rotation);
    }
}

pub(crate) fn update_scale_interpolation_start(
    mut query: Query<
        (&Transform, &mut ScaleEasingState),
        (With<ScaleInterpolation>, Without<NoScaleInterpolation>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.start = Some(transform.scale);
    }
}

pub(crate) fn update_scale_interpolation_end(
    mut query: Query<
        (&Transform, &mut ScaleEasingState),
        (With<ScaleInterpolation>, Without<NoScaleInterpolation>),
    >,
) {
    for (transform, mut easing) in &mut query {
        easing.end = Some(transform.scale);
    }
}
