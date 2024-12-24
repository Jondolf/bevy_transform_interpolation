//! Transform extrapolation is not a built-in feature in `bevy_transform_interpolation`, because it requires velocity.
//! However, it can be implemented in a relatively straightforward way on top of `TransformEasingPlugin`.
//!
//! This example showcases how `Transform` extrapolation can be used to make movement
//! appear smooth at fixed timesteps, and how it compares to `Transform` interpolation.
//!
//! Unlike `Transform` interpolation, which eases between the previous and current positions,
//! `Transform` extrapolation predicts future positions based on velocity. This makes movement
//! feel more responsive than interpolation, but it also produces jumpy results when the prediction is wrong,
//! such as when the velocity of an object suddenly changes.

use bevy::{
    color::palettes::{
        css::WHITE,
        tailwind::{CYAN_400, LIME_400, RED_400},
    },
    prelude::*,
};
use bevy_transform_interpolation::{
    prelude::*, RotationEasingState, TransformEasingSet, TranslationEasingState,
};

const MOVEMENT_SPEED: f32 = 250.0;
const ROTATION_SPEED: f32 = 2.0;

fn main() {
    let mut app = App::new();

    // Add the `TransformInterpolationPlugin` and `TransformExtrapolationPlugin` to the app to enable
    // transform interpolation and extrapolation.
    app.add_plugins((
        DefaultPlugins,
        TransformInterpolationPlugin::default(),
        // This is a custom plugin! See the implementation below.
        TransformExtrapolationPlugin,
    ));

    // Set the fixed timestep to just 5 Hz for demonstration purposes.
    app.insert_resource(Time::<Fixed>::from_hz(5.0));

    // Setup the scene and UI, and update text in `Update`.
    app.add_systems(Startup, (setup, setup_text))
        .add_systems(Update, (change_timestep, update_timestep_text));

    // Move entities in `FixedUpdate`. The movement should appear smooth for interpolated/extrapolated entities.
    app.add_systems(
        FixedUpdate,
        (flip_movement_direction.before(movement), movement, rotate),
    );

    // Run the app.
    app.run();
}

/// The linear velocity of an entity indicating its movement speed and direction.
#[derive(Component, Deref, DerefMut)]
struct LinearVelocity(Vec2);

/// The angular velocity of an entity indicating its rotation speed.
#[derive(Component, Deref, DerefMut)]
struct AngularVelocity(f32);

#[derive(Debug, Default)]
pub struct TransformExtrapolationPlugin;

impl Plugin for TransformExtrapolationPlugin {
    fn build(&self, app: &mut App) {
        // Reset the transform to the start of the extrapolation at the beginning of the fixed timestep
        // to match the true position from the end of the previous fixed tick.
        app.add_systems(
            FixedFirst,
            reset_extrapolation.before(TransformEasingSet::Reset),
        );

        // Update the start and end state of the extrapolation at the end of the fixed timestep.
        app.add_systems(
            FixedLast,
            update_easing_states.in_set(TransformEasingSet::UpdateEnd),
        );
    }

    fn finish(&self, app: &mut App) {
        // Add the `TransformEasingPlugin` if it hasn't been added yet.
        // It performs the actual easing based on the start and end states set by the extrapolation.
        if !app.is_plugin_added::<TransformEasingPlugin>() {
            app.add_plugins(TransformEasingPlugin::default());
        }
    }
}

/// Enables `Transform` extrapolation for an entity.
///
/// Only extrapolates the translation and rotation components of the transform
/// based on the `LinearVelocity` and `AngularVelocity` components.
#[derive(Component)]
#[require(TranslationEasingState, RotationEasingState)]
struct TransformExtrapolation;

/// Resets the transform to the start of the extrapolation at the beginning of the fixed timestep
/// to match the true position from the end of the previous fixed tick.
fn reset_extrapolation(
    mut query: Query<
        (
            &mut Transform,
            &TranslationEasingState,
            &RotationEasingState,
        ),
        With<TransformExtrapolation>,
    >,
) {
    for (mut transform, translation_easing, rotation_easing) in &mut query {
        if let Some(start) = translation_easing.start {
            transform.translation = start;
        }
        if let Some(start) = rotation_easing.start {
            transform.rotation = start;
        }
    }
}

/// Updates the start and end states of the extrapolation for the next fixed timestep.
fn update_easing_states(
    mut query: Query<
        (
            &Transform,
            &mut TranslationEasingState,
            &mut RotationEasingState,
            &LinearVelocity,
            &AngularVelocity,
        ),
        With<TransformExtrapolation>,
    >,
    time: Res<Time>,
) {
    let delta_secs = time.delta_secs();

    for (transform, mut translation_easing, mut rotation_easing, lin_vel, ang_vel) in &mut query {
        translation_easing.start = Some(transform.translation);
        rotation_easing.start = Some(transform.rotation);

        // Extrapolate the next state based on the current state and velocities.
        let next_translation = transform.translation + lin_vel.extend(0.0) * delta_secs;
        let next_rotation = transform.rotation * Quat::from_rotation_z(ang_vel.0 * delta_secs);

        // In 3D, with a `Vec3` angular velocity, the next rotation could be computed like this:
        //
        // let scaled_axis = ang_vel.0 * delta_secs;
        // let next_rotation = transform.rotation * Quat::from_scaled_axis(scaled_axis);

        translation_easing.end = Some(next_translation);
        rotation_easing.end = Some(next_rotation);
    }
}

// The rest of the code is scene setup, and largely the same as in the `interpolation.rs` example.

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Spawn a camera.
    commands.spawn(Camera2d);

    let mesh = meshes.add(Rectangle::from_length(60.0));

    // This entity uses transform interpolation.
    commands.spawn((
        Name::new("Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(CYAN_400)).clone()),
        Transform::from_xyz(-500.0, 120.0, 0.0),
        TransformInterpolation,
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
    ));

    // This entity uses transform extrapolation.
    commands.spawn((
        Name::new("Extrapolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(LIME_400)).clone()),
        Transform::from_xyz(-500.0, 00.0, 0.0),
        TransformExtrapolation,
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
    ));

    // This entity is simulated in `FixedUpdate` without any smoothing.
    commands.spawn((
        Name::new("No Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(RED_400)).clone()),
        Transform::from_xyz(-500.0, -120.0, 0.0),
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
    ));
}

/// Flips the movement directions of objects when they reach the left or right side of the screen.
fn flip_movement_direction(mut query: Query<(&Transform, &mut LinearVelocity)>) {
    for (transform, mut lin_vel) in &mut query {
        if transform.translation.x > 500.0 && lin_vel.0.x > 0.0 {
            lin_vel.0 = Vec2::new(-MOVEMENT_SPEED, 0.0);
        } else if transform.translation.x < -500.0 && lin_vel.0.x < 0.0 {
            lin_vel.0 = Vec2::new(MOVEMENT_SPEED, 0.0);
        }
    }
}

/// Changes the timestep of the simulation when the up or down arrow keys are pressed.
fn change_timestep(mut time: ResMut<Time<Fixed>>, keyboard_input: Res<ButtonInput<KeyCode>>) {
    if keyboard_input.pressed(KeyCode::ArrowUp) {
        let new_timestep = (time.delta_secs_f64() * 0.9).max(1.0 / 255.0);
        time.set_timestep_seconds(new_timestep);
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
        let new_timestep = (time.delta_secs_f64() * 1.1).min(1.0);
        time.set_timestep_seconds(new_timestep);
    }
}

/// Moves entities based on their `LinearVelocity`.
fn movement(mut query: Query<(&mut Transform, &LinearVelocity)>, time: Res<Time>) {
    let delta_secs = time.delta_secs();

    for (mut transform, lin_vel) in &mut query {
        transform.translation += lin_vel.extend(0.0) * delta_secs;
    }
}

/// Rotates entities based on their `AngularVelocity`.
fn rotate(mut query: Query<(&mut Transform, &AngularVelocity)>, time: Res<Time>) {
    let delta_secs = time.delta_secs();

    for (mut transform, ang_vel) in &mut query {
        transform.rotate_local_z(ang_vel.0 * delta_secs);
    }
}

#[derive(Component)]
struct TimestepText;

fn setup_text(mut commands: Commands) {
    let font = TextFont {
        font_size: 20.0,
        ..default()
    };

    commands
        .spawn((
            Text::new("Fixed Hz: "),
            TextColor::from(WHITE),
            font.clone(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                ..default()
            },
        ))
        .with_child((TimestepText, TextSpan::default()));

    commands.spawn((
        Text::new("Change Timestep With Up/Down Arrow"),
        TextColor::from(WHITE),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Interpolation"),
        TextColor::from(CYAN_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(50.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Extrapolation"),
        TextColor::from(LIME_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(75.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("No Interpolation"),
        TextColor::from(RED_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(100.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

fn update_timestep_text(
    mut text: Single<&mut TextSpan, With<TimestepText>>,
    time: Res<Time<Fixed>>,
) {
    let timestep = time.timestep().as_secs_f32().recip();
    text.0 = format!("{timestep:.2}");
}
