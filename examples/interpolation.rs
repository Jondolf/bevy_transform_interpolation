//! This example showcases how `Transform` interpolation can be used to make movement appear smooth at fixed timesteps.

use bevy::{
    color::palettes::{
        css::WHITE,
        tailwind::{CYAN_400, RED_400},
    },
    prelude::*,
    sprite::MaterialMesh2dBundle,
};
use bevy_transform_interpolation::{TransformInterpolationBundle, TransformInterpolationPlugin};

const MOVEMENT_SPEED: f32 = 250.0;
const ROTATION_SPEED: f32 = 2.0;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
        .insert_resource(Time::from_hz(5.0))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
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
    commands.spawn(Camera2dBundle::default());

    let rect_length = 60.0;
    let mesh = meshes.add(Rectangle::from_length(rect_length));

    // This entity uses transform interpolation.
    commands.spawn((
        Name::new("Interpolation"),
        MaterialMesh2dBundle {
            mesh: mesh.clone().into(),
            material: materials.add(Color::from(CYAN_400)).clone(),
            transform: Transform::from_xyz(-500.0, 60.0, 0.0),
            ..default()
        },
        TransformInterpolationBundle::default(),
        MovementDirection(Dir2::X),
    ));

    // This entity is simulated in `FixedUpdate` without any smoothing.
    commands.spawn((
        Name::new("No Interpolation"),
        MaterialMesh2dBundle {
            mesh: mesh.clone().into(),
            material: materials.add(Color::from(RED_400)).clone(),
            transform: Transform::from_xyz(-500.0, -60.0, 0.0),
            ..default()
        },
        MovementDirection(Dir2::X),
    ));
}

/// Flips the movement directions of objects when they reach the left or right side of the screen.
#[allow(clippy::type_complexity)]
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
        let new_timestep = (time.delta_seconds_f64() * 0.9).max(1.0 / 255.0);
        time.set_timestep_seconds(new_timestep);
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
        let new_timestep = (time.delta_seconds_f64() * 1.1).min(1.0);
        time.set_timestep_seconds(new_timestep);
    }
}

fn movement(mut query: Query<(&mut Transform, &MovementDirection)>, time: Res<Time>) {
    let delta_seconds = time.delta_seconds();

    for (mut transform, movement_direction) in &mut query {
        transform.translation += MOVEMENT_SPEED * movement_direction.extend(0.0) * delta_seconds;
    }
}

fn rotate(mut query: Query<&mut Transform, With<MovementDirection>>, time: Res<Time>) {
    let delta_seconds = time.delta_seconds();

    for mut transform in &mut query {
        transform.rotate_local_z(ROTATION_SPEED * delta_seconds);
    }
}

#[derive(Component)]
struct TimestepText;

fn setup_text(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "Fixed Hz: ",
            TextStyle {
                font: default(),
                font_size: 20.0,
                color: WHITE.into(),
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
        TimestepText,
    ));

    commands.spawn(
        TextBundle::from_section(
            "Change Timestep With Up/Down Arrow",
            TextStyle {
                font: default(),
                font_size: 20.0,
                color: WHITE.into(),
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        }),
    );

    commands.spawn(
        TextBundle::from_section(
            "Interpolation",
            TextStyle {
                font: default(),
                font_size: 20.0,
                color: CYAN_400.into(),
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(50.0),
            left: Val::Px(10.0),
            ..default()
        }),
    );

    commands.spawn(
        TextBundle::from_section(
            "No Interpolation",
            TextStyle {
                font: default(),
                font_size: 20.0,
                color: RED_400.into(),
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(75.0),
            left: Val::Px(10.0),
            ..default()
        }),
    );
}

fn update_timestep_text(time: Res<Time<Fixed>>, mut query: Query<&mut Text, With<TimestepText>>) {
    for mut text in &mut query {
        let timestep = time.timestep().as_secs_f32().recip();
        text.sections[0].value = format!("Fixed Hz: {timestep:.2}");
    }
}
