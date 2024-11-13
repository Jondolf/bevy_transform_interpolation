#![allow(clippy::type_complexity)]

use crate::*;
use bevy::prelude::*;

/// A component for enabling full [transform interpolation] for an entity.
/// Changes in translation, rotation, and scale are smoothed between [`FixedUpdate`] runs.
///
/// [transform interpolation]: crate
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationInterpolation, RotationInterpolation, ScaleInterpolation)]
pub struct TransformInterpolation;

/// Enables automatic translation interpolation for an entity.
///
/// Changes in translation are smoothed between [`FixedUpdate`] runs.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(TranslationEasingState)]
pub struct TranslationInterpolation;

/// Enables automatic rotation interpolation for an entity.
///
/// Changes in rotation are smoothed between [`FixedUpdate`] runs.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(RotationEasingState)]
pub struct RotationInterpolation;

/// Enables automatic scale interpolation for an entity.
///
/// Changes in scale are smoothed between [`FixedUpdate`] runs.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug, Default)]
#[require(ScaleEasingState)]
pub struct ScaleInterpolation;

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
