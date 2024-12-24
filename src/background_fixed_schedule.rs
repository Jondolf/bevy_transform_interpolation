use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings, ScheduleLabel};
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::{log::trace, prelude::World, time::Time};
use crossbeam_channel::Receiver;
use rand::{thread_rng, Rng};
use std::slice::IterMut;
use std::{collections::VecDeque, time::Duration};

/// The linear velocity of an entity indicating its movement speed and direction.
#[derive(Component, Debug, Clone, Deref, DerefMut)]
pub struct LinearVelocity(pub Vec2);

/// The angular velocity of an entity indicating its rotation speed.
#[derive(Component, Debug, Clone, Deref, DerefMut)]
pub struct AngularVelocity(pub f32);

#[derive(Component, Debug, Clone)]
pub struct ToMove;

/// Flips the movement directions of objects when they reach the left or right side of the screen.
fn flip_movement_direction(query: IterMut<(&mut Transform, &mut LinearVelocity)>) {
    for (transform, lin_vel) in query {
        if transform.translation.x > 500.0 && lin_vel.0.x > 0.0 {
            lin_vel.0 = Vec2::new(-lin_vel.x.abs(), 0.0);
        } else if transform.translation.x < -500.0 && lin_vel.0.x < 0.0 {
            lin_vel.0 = Vec2::new(lin_vel.x.abs(), 0.0);
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

pub struct BackgroundFixedUpdatePlugin;

impl Plugin for BackgroundFixedUpdatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            bevy::app::prelude::RunFixedMainLoop, // TODO: use a specific schedule for this, à la bevy's FixedMainLoop
            FixedMain::run_schedule,
        );

        // this handles checking for task completion, firing writeback schedules and spawning a new task.
        app.edit_schedule(FixedMain, |schedule| {
            schedule
                .add_systems(HandleTask::run_schedule)
                .set_build_settings(ScheduleBuildSettings {
                    ambiguity_detection: LogLevel::Error,
                    ..default()
                });
        });

        // those schedules are part of FixedMain
        app.init_schedule(PreWriteBack);
        app.edit_schedule(WriteBack, |schedule| {
            schedule
                .add_systems(handle_task)
                .set_build_settings(ScheduleBuildSettings {
                    ambiguity_detection: LogLevel::Error,
                    ..default()
                });
        });
        app.edit_schedule(SpawnTask, |schedule| {
            schedule
                .add_systems(spawn_task)
                .set_build_settings(ScheduleBuildSettings {
                    ambiguity_detection: LogLevel::Error,
                    ..default()
                });
        });
        app.edit_schedule(PostWriteBack, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..default()
            });
        });
    }
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

/// Spawn a new background task.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpawnTask;

/// Called after the propagation of the task result to the ECS.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PostWriteBack;

/// Schedule running [`PreWriteBack`], [`WriteBack`] and [`PostWriteBack`]
/// only if it received its data from the [`WorkTask`] present in the single Entity containing it.
///
/// This Schedule overrides [`Res<Time>`][Time] to be the task's time ([`Time<Fixed<MyTaskTime>>`]).
///
/// It's also responsible for spawning a new [`WorkTask`].
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
    pub fn run_schedule(world: &mut World, mut has_run_at_least_once: Local<bool>) {
        if !*has_run_at_least_once {
            world.run_system_cached(spawn_task);
            *has_run_at_least_once = true;
            return;
        }
        world
            .run_system_cached(finish_task_and_store_result)
            .unwrap();

        // Compute difference between task and render time.
        let clock = world.resource::<Time>().as_generic();
        let mut query = world.query::<(&mut TaskToRenderTime, &Timestep)>();
        let (mut task_to_render_time, timestep) = query.single_mut(world);
        task_to_render_time.diff += clock.delta().as_secs_f64();
        if task_to_render_time.diff < timestep.timestep.as_secs_f64() {
            // Task is too far ahead, we should not read the simulation.
            //world.run_system_cached(spawn_task);
            info!("Task is too far ahead, we should not read the simulation.");
            return;
        }
        let simulated_time = {
            let mut query = world.query::<&TaskResults>();
            let task_result = query.single(world).results.front();
            task_result.map(|task_result| task_result.result.simulated_time)
        };
        let Some(simulated_time) = simulated_time else {
            //world.run_system_cached(spawn_task);
            info!("No task result found.");
            return;
        };
        let mut query = world.query::<&mut TaskToRenderTime>();
        let mut task_to_render_time = query.single_mut(world);
        task_to_render_time.diff -= simulated_time.as_secs_f64();
        let _ = world.try_schedule_scope(FixedMain, |world, schedule| {
            // Advance simulation.
            info!("Running FixedMain schedule");
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
        let _ = world.try_schedule_scope(SpawnTask, |world, schedule| {
            schedule.run(world);
        });
        let _ = world.try_schedule_scope(PostWriteBack, |world, schedule| {
            schedule.run(world);
        });
    }
}

/// This system spawns a [`WorkTask`] is none are ongoing.
/// The task simulate computationally intensive work that potentially spans multiple frames/ticks.
///
/// A separate system, [`handle_tasks`], will poll the spawned tasks on subsequent
/// frames/ticks, and use the results to spawn cubes
pub fn spawn_task(
    mut commands: Commands,
    q_context: Query<(Entity, &TaskToRenderTime, &Timestep, Has<WorkTask>)>,
    q_transforms: Query<(Entity, &Transform, &LinearVelocity, &AngularVelocity), With<ToMove>>,
    virtual_time: Res<Time<Virtual>>,
) {
    let Ok((entity_ctx, task_to_render_time, timestep, has_work)) = q_context.get_single() else {
        info!("No correct entity found.");
        return;
    };
    if has_work {
        info!("A task is ongoing.");
        return;
    }
    let timestep = timestep.timestep;

    // TODO: tweak this on user side, to allow the simulation to catch up with the render time.
    let mut substep_count = 1;

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
            profiling::scope!("Rapier physics simulation");
            // Simulate an expensive task

            let to_simulate = simulated_time.as_millis() as u64;
            std::thread::sleep(Duration::from_millis(thread_rng().gen_range(200..201)));

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
    mut task_results: Query<(&mut TaskResults, &mut TaskToRenderTime)>,
    mut q_transforms: Query<(&mut Transform, &mut LinearVelocity)>,
) {
    for (mut results, mut task_to_render) in task_results.iter_mut() {
        let Some(task) = results.results.pop_front() else {
            continue;
        };
        // Apply transform changes.
        info!(
            "handle_task: simulated_time: {:?}",
            task.result.simulated_time
        );
        for (entity, new_transform, new_lin_vel, _) in task.result.transforms.iter() {
            if let Ok((mut transform, mut lin_vel)) = q_transforms.get_mut(*entity) {
                *transform = *new_transform;
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
