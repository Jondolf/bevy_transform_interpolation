# `bevy_transform_interpolation`

A general-purpose `Transform` interpolation plugin for fixed timesteps for the [Bevy game engine](https://bevyengine.org).

## What Is This For?

A lot of gameplay logic and movement systems typically use a fixed timestep to produce consistent and stable behavior
regardless of the frame rate. Notable examples include physics simulation and character movement.

However, this can make movement appear choppy, especially on displays with a high refresh rate.
To achieve visually smooth movement while using a fixed timestep, the visual transform must be smoothed
independently of the "true" gameplay transform.

The most common way to do this is to use **transform interpolation**, which interpolates movement from the previous
state to the current state. This could be done by storing the current and old gameplay positions in their own components
and interpolating `Transform` using them:

```rust
use bevy::prelude::*;

#[derive(Component, Deref, DerefMut)]
struct Position(Vec3);

#[derive(Component, Deref, DerefMut)]
struct OldPosition(Vec3);

// Runs in `Update` or `PostUpdate`.
fn interpolate_transforms(
    mut query: Query<(&mut Transform, &Position, &OldPosition)>,
    fixed_time: Res<Time<Fixed>>
) {
    // How much of a "partial timestep" has accumulated since the last fixed timestep run.
    // Between `0.0` and `1.0`.
    let overstep = fixed_time.overstep_fraction();

    for (mut transform, position, old_position) in &mut query {
        // Linearly interpolate the translation from the old position to the current one.
        transform.translation = old_position.lerp(position.0, overstep_fraction);
    }
}
```

In fact, you could simply plug the above implementation into your own application if you wanted to!

However, it requires you to use `Position` for gameplay logic, and to manage `OldPosition` somewhere.
This can be annoying, and is incompatibile with third party libraries that expect to be able to modify
the transform directly.

`bevy_transform_interpolation` aims to be a drop-in solution that allows easy and efficient transform interpolation,
while still allowing the usage of `Transform` for gameplay logic. It should be automatically compatible with physics engines
such as [Avian](https://github.com/Jondolf/avian) and [`bevy_rapier`](https://github.com/dimforge/bevy_rapier), as long as
the simulation is run in `FixedUpdate` or `FixedPostUpdate`.

## Usage

First, add `bevy_transform_interpolation` to your dependencies in `Cargo.toml`:

```toml
[dependencies]
bevy_transform_interpolation = { git = "https://github.com/Jondolf/bevy_transform_interpolation" }
```

Next, add the `TransformInterpolationPlugin`:

```rust
use bevy::prelude::*;
use bevy_transform_interpolation::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TransformInterpolationPlugin::default()))
        // ...other plugins, resources, and systems
        .run();
}
```

Transform interpolation can be enabled very granularly in `bevy_transform_interpolation`.
You can choose to interpolate transform, rotation, or scale individually, or use any combination of them:

```rust
use bevy::prelude::*;
use bevy_transform_interpolation::*;

fn setup(mut commands: Commands) {
    // Only interpolate translation.
    commands.spawn((TransformBundle::default(), TranslationInterpolation));
    
    // Only interpolate rotation.
    commands.spawn((TransformBundle::default(), RotationInterpolation));
    
    // Only interpolate scale.
    commands.spawn((TransformBundle::default(), ScaleInterpolation));
    
    // Interpolate translation and rotation, but not scale.
    commands.spawn((
        TransformBundle::default(),
        TranslationInterpolation,
        RotationInterpolation,
    ));
    
    // Interpolate the entire transform: translation, rotation, and scale.
    // The components can be added individually, or using the `TransformInterpolation` component.
    commands.spawn((
        TransformBundle::default(),
        TransformInterpolation,
    ));
}
```

You can also enable transform interpolation globally for *all* entities that have a `Transform`
by configuring the `TransformInterpolationPlugin`:

```rust
use bevy::prelude::*;
use bevy_transform_interpolation::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            // Interpolate translation and rotation, but not scale.
            TransformInterpolationPlugin {
                global_translation_interpolation: true,
                global_rotation_interpolation: true,
                global_scale_interpolation: false,
            },
        ))
        // ...other plugins, resources, and systems
        .run();
}
```

If interpolation is enabled globally, it can still be disabled for individual entities using the `NoTranslationInterpolation`,
`NoRotationInterpolation`, and `NoScaleInterpolation` components.

Now, any changes made to `Transform` in `FixedPreUpdate`, `FixedUpdate`, or `FixedPostUpdate` will automatically
be smoothed in between the fixed timesteps for entities that have transform interpolation enabled.

Changing `Transform` manually in any schedule that *doesn't* use a fixed timestep is also supported,
but it is equivalent to teleporting, and disables interpolation for the entity for the remainder of that fixed timestep.

## How Does It Work?

Internally, `bevy_transform_interpolation` simply maintains components that store the `start` and `end` of the interpolation.
For example, translation uses the following component for easing the movement:

```rust
pub struct TranslationEasingState {
    pub start: Option<Vec3>,
    pub end: Option<Vec3>,
}
```

- At the start of the `FixedFirst` schedule, the states are reset to `None`.
- In `FixedFirst`, for every entity with the `TranslationInterpolation` component, `start` is set to the current `Transform`.
- In `FixedLast`, for every entity with the `TranslationInterpolation` component, `end` is set to the current `Transform`.

This way, `start` represents the "old" state, while `end` represents the "new" state after changes have been made to `Transform`
in between `FixedFirst` and `FixedLast`. Rotation and scale are handled similarly.

The easing is then performed in `PostUpdate`, before Bevy's transform propagation systems. If the `Transform` is detected to have changed
since the last easing run but *outside* of the fixed timestep schedules, the easing is reset to `None` to prevent overwriting the change.

Note that the core easing logic and components are intentionally not tied to interpolation directly.
A physics engine could implement **transform extrapolation** using velocity and the same easing functionality,
supplying its own `TranslationExtrapolation` and `RotationExtrapolation` components.

## Caveats

- In cases where the previous or current gameplay transform are already stored separately from `Transform`,
  storing them in the easing states as well may be redundant. Although it *is* still useful for allowing
  `Transform` to be modified directly and for wider compatibility with the ecosystem.
- Transform extrapolation is currently not supported as a built-in feature, as it typically requires a velocity
  for the prediction of the next state. However, it could be supported by external libraries such as physics engines
  in a similar way to `src/interpolation.rs`, and simply updating the `start` and `end` states differently.
- Large angular velocities may cause visual artifacts, as the interpolation follows the shortest path.
  A physics engine could handle this properly.

## License

`bevy_transform_interpolation` is free and open source. All code in this repository is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.
