//! This example showcases how `Transform` interpolation can be used to make movement
//! appear smooth at fixed timesteps.
//!
//! `Transform` interpolation updates `Transform` at every frame in between
//! fixed ticks to smooth out the visual result. The interpolation is done
//! from the previous positions to the current positions, which keeps movement smooth,
//! but has the downside of making movement feel slightly delayed as the rendered
//! result lags slightly behind the true positions.
//!
//! For an example of how transform extrapolation could be implemented instead,
//! see `examples/extrapolation.rs`.

use bevy::{
    color::palettes::{
        css::WHITE,
        tailwind::{CYAN_400, RED_400},
    },
    ecs::schedule::{LogLevel, ScheduleBuildSettings, ScheduleLabel},
    prelude::*,
    tasks::AsyncComputeTaskPool,
};
use bevy_transform_interpolation::{
    prelude::*, RotationEasingState, ScaleEasingState, TransformEasingSet, TranslationEasingState,
};
use crossbeam_channel::Receiver;
use rand::{thread_rng, Rng};
use std::{collections::VecDeque, slice::IterMut, time::Duration};

const MOVEMENT_SPEED: f32 = 250.0;
const ROTATION_SPEED: f32 = 2.0;

// TODO: update this time to use it correctly.
// See https://github.com/bevyengine/bevy/blob/d4b07a51149c4cc69899f7424df473ff817fe324/crates/bevy_time/src/fixed.rs#L241

fn main() {
    let mut app = App::new();

    // Add the `TransformInterpolationPlugin` to the app to enable transform interpolation.
    app.add_plugins(DefaultPlugins);

    // Set the fixed timestep to just 5 Hz for demonstration purposes.

    // Setup the scene and UI, and update text in `Update`.
    app.add_systems(Startup, (setup, setup_text)).add_systems(
        bevy::app::prelude::RunFixedMainLoop,
        (
            change_timestep,
            update_timestep_text,
            update_diff_to_render_text,
        ),
    );

    // This runs every frame to poll if our task was done.
    app.add_systems(
        bevy::app::prelude::RunFixedMainLoop, // TODO: use a specific schedule for this, à la bevy's FixedMainLoop
        task_schedule::FixedMain::run_schedule,
    );

    // this handles checking for task completion, firing writeback schedules and spawning a new task.
    app.edit_schedule(task_schedule::FixedMain, |schedule| {
        schedule
            .add_systems(task_schedule::HandleTask::run_schedule)
            .set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..default()
            });
    });

    // those schedules are part of FixedMain
    app.init_schedule(task_schedule::PreWriteBack);
    app.edit_schedule(task_schedule::WriteBack, |schedule| {
        schedule
            .add_systems((handle_task,))
            .set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..default()
            });
    });
    app.edit_schedule(task_schedule::PostWriteBack, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..default()
        });
    });

    app.add_systems(
        bevy::app::prelude::RunFixedMainLoop,
        (ease_translation_lerp, ease_rotation_slerp, ease_scale_lerp)
            .in_set(TransformEasingSet::Ease),
    );
    // this will spawn a new task if needed.
    app.edit_schedule(task_schedule::MaybeSpawnTask, |schedule| {
        schedule
            .add_systems(spawn_task)
            .set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..default()
            });
    });

    // Run the app.
    app.run();
}
/// Eases the translations of entities with linear interpolation.
fn ease_translation_lerp(
    mut query: Query<(&mut Transform, &TranslationEasingState)>,
    time: Query<(&TaskToRenderTime, &Timestep, &LastTaskTimings)>,
) {
    let Ok((time, timestep, last_task_timing)) = time.get_single() else {
        return;
    };
    let overstep = (time.diff.max(0.0)
        / (timestep.timestep - last_task_timing.render_time_elapsed_during_the_simulation)
            .as_secs_f64())
    .min(1.0) as f32;
    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.translation = start.lerp(end, overstep);
        }
    });
}

/// Eases the rotations of entities with spherical linear interpolation.
fn ease_rotation_slerp(
    mut query: Query<(&mut Transform, &RotationEasingState)>,
    time: Query<(&TaskToRenderTime, &Timestep)>,
) {
    let Ok((time, timestep)) = time.get_single() else {
        return;
    };
    let overstep = (time.diff.max(0.0) / timestep.timestep.as_secs_f64()).min(1.0) as f32;

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
fn ease_scale_lerp(
    mut query: Query<(&mut Transform, &ScaleEasingState)>,
    time: Query<(&TaskToRenderTime, &Timestep)>,
) {
    let Ok((time, timestep)) = time.get_single() else {
        return;
    };
    let overstep = (time.diff.max(0.0) / timestep.timestep.as_secs_f64()).min(1.0) as f32;

    query.iter_mut().for_each(|(mut transform, interpolation)| {
        if let (Some(start), Some(end)) = (interpolation.start, interpolation.end) {
            transform.scale = start.lerp(end, overstep);
        }
    });
}

/// The linear velocity of an entity indicating its movement speed and direction.
#[derive(Component, Debug, Deref, DerefMut, Clone)]
pub struct LinearVelocity(Vec2);

/// The angular velocity of an entity indicating its rotation speed.
#[derive(Component, Debug, Deref, DerefMut, Clone)]
pub struct AngularVelocity(f32);

#[derive(Component, Debug, Clone)]
struct ToMove;

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Spawn a camera.
    commands.spawn(Camera2d);

    let mesh = meshes.add(Rectangle::from_length(60.0));

    commands.spawn((
        TaskToRenderTime::default(),
        Timestep {
            timestep: Duration::from_secs_f32(0.5),
        },
        TaskResults::default(),
    ));

    // This entity uses transform interpolation.
    commands.spawn((
        Name::new("Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(CYAN_400)).clone()),
        Transform::from_xyz(-500.0, 60.0, 0.0),
        TransformInterpolation,
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
        ToMove,
    ));

    // This entity is simulated in `FixedUpdate` without any smoothing.
    commands.spawn((
        Name::new("No Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(RED_400)).clone()),
        Transform::from_xyz(-500.0, -60.0, 0.0),
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
        ToMove,
    ));
}

/// Changes the timestep of the simulation when the up or down arrow keys are pressed.
fn change_timestep(mut time: Query<&mut Timestep>, keyboard_input: Res<ButtonInput<KeyCode>>) {
    let mut time = time.single_mut();
    if keyboard_input.pressed(KeyCode::ArrowUp) {
        let new_timestep = (time.timestep.as_secs_f64() * 0.9).max(1.0 / 255.0);
        time.timestep = Duration::from_secs_f64(new_timestep);
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
        let new_timestep = (time.timestep.as_secs_f64() * 1.1)
            .min(1.0)
            .max(1.0 / 255.0);
        time.timestep = Duration::from_secs_f64(new_timestep);
    }
}

/// Flips the movement directions of objects when they reach the left or right side of the screen.
fn flip_movement_direction(query: IterMut<(&mut Transform, &mut LinearVelocity)>) {
    for (transform, lin_vel) in query {
        if transform.translation.x > 500.0 && lin_vel.0.x > 0.0 {
            lin_vel.0 = Vec2::new(-MOVEMENT_SPEED, 0.0);
        } else if transform.translation.x < -500.0 && lin_vel.0.x < 0.0 {
            lin_vel.0 = Vec2::new(MOVEMENT_SPEED, 0.0);
        }
    }
}

/// Moves entities based on their `LinearVelocity`.
fn movement(query: IterMut<(&mut Transform, &mut LinearVelocity)>, delta: Duration) {
    let delta_secs = delta.as_secs_f32();
    for (transform, lin_vel) in query {
        transform.translation += lin_vel.extend(0.0) * delta_secs;
    }
}

/// Rotates entities based on their `AngularVelocity`.
fn rotate(query: IterMut<(&mut Transform, &mut AngularVelocity)>, delta: Duration) {
    let delta_secs = delta.as_secs_f32();
    for (transform, ang_vel) in query {
        transform.rotate_local_z(ang_vel.0 * delta_secs);
    }
}

#[derive(Component)]
struct TimestepText;

#[derive(Component)]
struct TaskToRenderTimeText;

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

    commands
        .spawn((
            Text::new("Diff to render time: "),
            TextColor::from(WHITE),
            font.clone(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(100.0),
                left: Val::Px(10.0),
                ..default()
            },
        ))
        .with_child((TaskToRenderTimeText, TextSpan::default()));
}

fn update_timestep_text(
    mut text: Single<&mut TextSpan, With<TimestepText>>,
    time: Query<&Timestep>,
) {
    let timestep = time.single().timestep.as_secs_f32().recip();
    text.0 = format!("{timestep:.2}");
}

fn update_diff_to_render_text(
    mut text: Single<&mut TextSpan, With<TaskToRenderTimeText>>,
    task_to_render: Single<&TaskToRenderTime>,
) {
    text.0 = format!("{:.2}", task_to_render.diff);
}

pub mod task_schedule {

    use bevy::{
        ecs::schedule::ScheduleLabel,
        log::{info, trace},
        prelude::{SystemSet, World},
        time::Time,
    };

    use crate::TaskToRenderTime;

    #[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum FixedMainLoop {
        Before,
        During,
        After,
    }

    /// Executes before the task result is propagated to the ECS.
    #[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PreWriteBack;

    /// Propagates the task result to the ECS.
    #[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct WriteBack;

    /// Called after the propagation of the task result to the ECS.
    #[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PostWriteBack;

    /// Called once to start a task, then after receiving each task result.
    #[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MaybeSpawnTask;

    /// Schedule running [`PreWriteBack`], [`WriteBack`] and [`PostWriteBack`]
    /// only if it received its data from the [`super::WorkTask`] present in the single Entity containing it.
    ///
    /// This Schedule overrides [`Res<Time>`][Time] to be the task's time ([`Time<Fixed<MyTaskTime>>`]).
    ///
    /// It's also responsible for spawning a new [`super::WorkTask`].
    ///
    /// This Schedule does not support multiple Entities with the same `Task` component.
    // TODO: Schedule as entities might be able to support multiple entities?
    ///
    /// This works similarly to [`bevy's FixedMain`][bevy::app::FixedMain],
    /// but it is not blocked by the render loop.
    #[derive(Debug, Hash, PartialEq, Eq, Clone, ScheduleLabel)]
    pub struct FixedMain;

    impl FixedMain {
        /// A system that runs the [`SingleTaskSchedule`] if the task was done.
        pub fn run_schedule(world: &mut World) {
            world
                .run_system_cached(crate::finish_task_and_store_result)
                .unwrap();

            // Compute difference between task and render time.
            let clock = world.resource::<Time>().as_generic();
            let mut query = world.query::<(&mut TaskToRenderTime, &super::Timestep)>();
            let (mut task_to_render_time, timestep) = query.single_mut(world);
            task_to_render_time.diff += clock.delta().as_secs_f64();
            // should we apply deferred commands?
            if task_to_render_time.diff <= timestep.timestep.as_secs_f64() {
                // Task is too far ahead, we should not read the simulation.
                return;
            }
            let simulated_time = {
                let mut query = world.query::<&crate::TaskResults>();
                let task_result = query.single(world).results.front();
                task_result.map(|task_result| task_result.result.simulated_time)
            };
            let Some(simulated_time) = simulated_time else {
                let mut query = world.query::<&crate::LastTaskTimings>();
                if query.get_single(world).is_err() {
                    world.run_schedule(MaybeSpawnTask);
                }
                return;
            };
            let mut query = world.query::<&mut TaskToRenderTime>();
            let mut task_to_render_time = query.single_mut(world);
            task_to_render_time.diff -= simulated_time.as_secs_f64();
            let _ = world.try_schedule_scope(FixedMain, |world, schedule| {
                // Advance simulation.
                trace!("Running FixedMain schedule");
                schedule.run(world);

                // If physics is paused, reset delta time to stop simulation
                // unless users manually advance `Time<Physics>`.
                /*if is_paused {
                    world
                        .resource_mut::<Time<Physics>>()
                        .advance_by(Duration::ZERO);
                }
                */
            });
            // PROBLEM: This is outside of our fixed update, so we're reading the interpolated transforms.
            // This is unacceptable because that's not our ground truth.
            //world.run_schedule(MaybeSpawnTask);
        }
    }

    /// Schedule handling a single task.
    #[derive(Debug, Hash, PartialEq, Eq, Clone, ScheduleLabel)]
    pub struct HandleTask;

    impl HandleTask {
        pub fn run_schedule(world: &mut World) {
            let _ = world.try_schedule_scope(PreWriteBack, |world, schedule| {
                schedule.run(world);
            });
            let _ = world.try_schedule_scope(WriteBack, |world, schedule| {
                schedule.run(world);
            });
            let _ = world.try_schedule_scope(MaybeSpawnTask, |world, schedule| {
                schedule.run(world);
            });
            let _ = world.try_schedule_scope(PostWriteBack, |world, schedule| {
                schedule.run(world);
            });
        }
    }
}

///
/// The task inside this component is polled by the system [`handle_tasks`].
///
/// Any changes to [`Transform`]s being modified by the task will be overridden when the task finishes.
///
/// This component is removed when the task is done
#[derive(Component, Debug)]
pub struct WorkTask {
    /// The time in seconds at which we started the simulation, as reported by the used render time [`Time::elapsed`].
    pub started_at_render_time: Duration,
    /// Amount of frames elapsed since the simulation started.
    pub update_frames_elapsed: u32,
    /// The channel end to receive the simulation result.
    pub recv: Receiver<TaskResultRaw>,
}

/// The result of a task to be handled.
#[derive(Debug, Default)]
pub struct TaskResultRaw {
    pub transforms: Vec<(Entity, Transform, LinearVelocity, AngularVelocity)>,
    /// The duration in seconds **simulated** by the simulation.
    ///
    /// This is different from the real time it took to simulate the physics.
    ///
    /// It is needed to synchronize the simulation with the render time.
    pub simulated_time: Duration,
}

/// The result of a task to be handled.
#[derive(Debug, Default)]
pub struct TaskResult {
    pub result: TaskResultRaw,
    pub render_time_elapsed_during_the_simulation: Duration,
    /// The time at which we started the simulation, as reported by the used render time [`Time::elapsed`].
    pub started_at_render_time: Duration,
    /// Amount of frames elapsed since the simulation started.
    pub update_frames_elapsed: u32,
}
/// The result of last task result, helpful for interpolation.
#[derive(Debug, Default, Component)]
pub struct LastTaskTimings {
    pub render_time_elapsed_during_the_simulation: Duration,
    /// The time at which we started the simulation, as reported by the used render time [`Time::elapsed`].
    pub started_at_render_time: Duration,
}

#[derive(Debug, Default, Component)]
pub struct RealTransform(pub Transform);

/// The result of a task to be handled.
#[derive(Debug, Default, Component)]
pub struct TaskResults {
    /// The results of the tasks.
    ///
    /// This is a queue because we might be spawning a new task while another has not been processed yet.
    ///
    /// To avoid overwriting the results, we keep them in a queue.
    pub results: VecDeque<TaskResult>,
}

/// Difference between tasks and rendering time
#[derive(Component, Default, Reflect, Clone)]
pub struct TaskToRenderTime {
    /// Difference in seconds between tasks and rendering time.
    ///
    /// We don't use [`Duration`] because it can be negative.
    pub diff: f64,
    /// Amount of rendering frames last task took.
    pub last_task_frame_count: u32,
}

/// Difference between tasks and rendering time
#[derive(Component, Default, Reflect, Clone)]
pub struct Timestep {
    pub timestep: Duration,
}

/// This system spawns a [`WorkTask`] is none are ongoing.
/// The task simulate computationally intensive work that potentially spans multiple frames/ticks.
///
/// A separate system, [`handle_tasks`], will poll the spawned tasks on subsequent
/// frames/ticks, and use the results to spawn cubes
pub(crate) fn spawn_task(
    mut commands: Commands,
    q_context: Query<(
        Entity,
        &TaskToRenderTime,
        &Timestep,
        Has<WorkTask>,
        &TaskResults,
    )>,
    q_transforms: Query<(Entity, &mut Transform, &LinearVelocity, &AngularVelocity), With<ToMove>>,
    virtual_time: Res<Time<Virtual>>,
) {
    let Ok((entity_ctx, task_to_render_time, timestep, has_work, results)) = q_context.get_single()
    else {
        info!("No correct entity found.");
        return;
    };
    if has_work {
        info!("A task is ongoing.");
        return;
    }
    let timestep = timestep.timestep;

    // We are not impacting task to render diff yet, because the task has not run yet.
    // Ideally, this should be driven from user code.
    let mut sim_to_render_time = task_to_render_time.clone();

    let mut substep_count = 1;
    /*while sim_to_render_time.diff > timestep.as_secs_f64() {
        sim_to_render_time.diff -= timestep.as_secs_f64();
        substep_count += 1;
    }
    if substep_count == 0 {
        info!("No substeps needed.");
        return;
    }*/

    let mut transforms_to_move: Vec<(Entity, Transform, LinearVelocity, AngularVelocity)> =
        q_transforms
            .iter()
            .map(|(entity, transform, lin_vel, ang_vel)| {
                (entity, transform.clone(), lin_vel.clone(), ang_vel.clone())
            })
            .collect();
    let (sender, recv) = crossbeam_channel::unbounded();

    let thread_pool = AsyncComputeTaskPool::get();
    thread_pool
        .spawn(async move {
            let simulated_time = timestep * substep_count;

            info!(
                "Let's spawn a simulation task for time: {:?}",
                simulated_time
            );
            profiling::scope!("Task ongoing");
            // Simulate an expensive task

            let to_simulate = simulated_time.as_millis() as u64;
            std::thread::sleep(Duration::from_millis(thread_rng().gen_range(100..101)));

            // Move entities in a fixed amount of time. The movement should appear smooth for interpolated entities.
            flip_movement_direction(
                transforms_to_move
                    .iter_mut()
                    .map(|(_, transform, lin_vel, _)| (transform, lin_vel))
                    .collect::<Vec<_>>()
                    .iter_mut(),
            );
            movement(
                transforms_to_move
                    .iter_mut()
                    .map(|(_, transform, lin_vel, _)| (transform, lin_vel))
                    .collect::<Vec<_>>()
                    .iter_mut(),
                simulated_time,
            );
            rotate(
                transforms_to_move
                    .iter_mut()
                    .map(|(_, transform, _, ang_vel)| (transform, ang_vel))
                    .collect::<Vec<_>>()
                    .iter_mut(),
                simulated_time,
            );
            let mut result = TaskResultRaw::default();
            result.transforms = transforms_to_move;
            result.simulated_time = simulated_time;
            let _ = sender.send(result);
        })
        .detach();

    commands.entity(entity_ctx).insert(WorkTask {
        recv,
        started_at_render_time: virtual_time.elapsed(),
        update_frames_elapsed: 0,
    });
}

/// This system queries for `Task<RapierSimulation>` component. It polls the
/// task, if it has finished, it removes the [`WorkTask`] component from the entity,
/// and adds a [`TaskResult`] component.
///
/// This expects only 1 task at a time.
pub(crate) fn finish_task_and_store_result(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut q_tasks: Query<(Entity, &mut WorkTask, &mut TaskResults)>,
) {
    let Ok((e, mut task, mut results)) = q_tasks.get_single_mut() else {
        return;
    };
    task.update_frames_elapsed += 1;

    let mut handle_result = |task_result: TaskResultRaw| {
        commands.entity(e).remove::<WorkTask>();
        results.results.push_back(TaskResult {
            result: task_result,
            render_time_elapsed_during_the_simulation: dbg!(time.elapsed())
                - dbg!(task.started_at_render_time),
            started_at_render_time: task.started_at_render_time,
            update_frames_elapsed: task.update_frames_elapsed,
        });
        info!("Task finished!");
    };
    // TODO: configure this somehow.
    if task.update_frames_elapsed > 60 {
        // Do not tolerate more delay over the rendering: block on the result of the simulation.
        if let Some(result) = task.recv.recv().ok() {
            handle_result(result);
        }
    } else {
        if let Some(result) = task.recv.try_recv().ok() {
            handle_result(result);
        }
    }
}

pub(crate) fn handle_task(
    mut commands: Commands,
    mut task_results: Query<(Entity, &mut TaskResults, &mut TaskToRenderTime)>,
    mut q_transforms: Query<(&mut RealTransform, &mut LinearVelocity)>,
) {
    for (e, mut results, mut task_to_render) in task_results.iter_mut() {
        let Some(task) = results.results.pop_front() else {
            continue;
        };
        commands.entity(e).insert(LastTaskTimings {
            render_time_elapsed_during_the_simulation: task
                .render_time_elapsed_during_the_simulation,
            started_at_render_time: task.started_at_render_time,
        });
        // Apply transform changes.
        info!(
            "handle_task: simulated_time: {:?}",
            task.result.simulated_time
        );
        for (entity, new_transform, new_lin_vel, _) in task.result.transforms.iter() {
            if let Ok((mut transform, mut lin_vel)) = q_transforms.get_mut(*entity) {
                transform.0 = *new_transform;
                *lin_vel = new_lin_vel.clone();
            }
        }
        //let diff_this_frame = dbg!(task.render_time_elapsed_during_the_simulation.as_secs_f64())
        //    - dbg!(task.result.simulated_time.as_secs_f64());
        //task_to_render.diff += dbg!(diff_this_frame);
        //task_to_render.diff += dbg!(diff_this_frame);
        task_to_render.last_task_frame_count = task.update_frames_elapsed;
    }
}
