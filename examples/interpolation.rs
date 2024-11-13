//! This example showcases how `Transform` interpolation can be used to make movement appear smooth at fixed timesteps.

use bevy::{
    color::palettes::{
        css::WHITE,
        tailwind::{CYAN_400, RED_400},
    },
    prelude::*,
};
use bevy_transform_interpolation::{TransformInterpolation, TransformInterpolationPlugin};

const MOVEMENT_SPEED: f32 = 250.0;
const ROTATION_SPEED: f32 = 2.0;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
        .insert_resource(Time::<Fixed>::from_hz(5.0))
        .add_systems(Startup, (setup, setup_text))
        .add_systems(Update, (change_timestep, update_timestep_text))
        .add_systems(
            FixedUpdate,
            (flip_movement_direction.before(movement), movement, rotate),
        )
        .run();
}

#[derive(Component, Deref, DerefMut)]
struct MovementDirection(Dir2);

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn(Camera2d);

    let rect_length = 60.0;
    let mesh = meshes.add(Rectangle::from_length(rect_length));

    // This entity uses transform interpolation.
    commands.spawn((
        Name::new("Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(CYAN_400)).clone()),
        Transform::from_xyz(-500.0, 60.0, 0.0),
        TransformInterpolation,
        MovementDirection(Dir2::X),
    ));

    // This entity is simulated in `FixedUpdate` without any smoothing.
    commands.spawn((
        Name::new("No Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(RED_400)).clone()),
        Transform::from_xyz(-500.0, -60.0, 0.0),
        MovementDirection(Dir2::X),
    ));
}

/// Flips the movement directions of objects when they reach the left or right side of the screen.
fn flip_movement_direction(mut query: Query<(&GlobalTransform, &mut MovementDirection)>) {
    for (transform, mut dir) in &mut query {
        let translation = transform.translation();
        if translation.x > 500.0 && dir.0.x > 0.0 {
            dir.0 = Dir2::NEG_X;
        } else if translation.x < -500.0 && dir.0.x < 0.0 {
            dir.0 = Dir2::X;
        }
    }
}

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

fn movement(mut query: Query<(&mut Transform, &MovementDirection)>, time: Res<Time>) {
    let delta_secs = time.delta_secs();

    for (mut transform, movement_direction) in &mut query {
        transform.translation += MOVEMENT_SPEED * movement_direction.extend(0.0) * delta_secs;
    }
}

fn rotate(mut query: Query<&mut Transform, With<MovementDirection>>, time: Res<Time>) {
    let delta_secs = time.delta_secs();

    for mut transform in &mut query {
        transform.rotate_local_z(ROTATION_SPEED * delta_secs);
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
        Text::new("No Interpolation"),
        TextColor::from(RED_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(75.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

fn update_timestep_text(
    time: Res<Time<Fixed>>,
    mut text: Single<&mut TextSpan, With<TimestepText>>,
) {
    let timestep = time.timestep().as_secs_f32().recip();
    text.0 = format!("{timestep:.2}");
}
