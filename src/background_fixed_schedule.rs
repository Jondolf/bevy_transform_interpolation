use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings, ScheduleLabel};
use bevy::ecs::world;
use bevy::log::tracing_subscriber::fmt::time;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::{log::trace, prelude::World, time::Time};
use crossbeam_channel::Receiver;
use rand::{thread_rng, Rng};
use std::default;
use std::slice::IterMut;
use std::{collections::VecDeque, time::Duration};

///
/// The task inside this component is polled by the system [`handle_tasks`].
///
/// Any changes to [`Transform`]s being modified by the task will be overridden when the task finishes.
///
/// This component is removed when the task is done
#[derive(Component, Debug)]
pub struct WorkTask<T: TaskWorkerTrait + Send + Sync> {
    /// The time in seconds at which we started the simulation, as reported by the used render time [`Time::elapsed`].
    pub started_at_render_time: Duration,
    /// Amount of frames elapsed since the simulation started.
    pub update_frames_elapsed: u32,
    /// The channel end to receive the simulation result.
    pub recv: Receiver<TaskResultRaw<T>>,
}

/// The result of a task to be handled.
#[derive(Debug, Default)]
pub struct TaskResultRaw<T: TaskWorkerTrait + Send + Sync> {
    pub transforms: T::TaskResultPure,
    /// The duration in seconds **simulated** by the simulation.
    ///
    /// This is different from the real time it took to simulate the physics.
    ///
    /// It is needed to synchronize the simulation with the render time.
    pub simulated_time: Duration,
}

/// The result of a task to be handled.
pub struct TaskResult<T: TaskWorkerTrait + Send + Sync> {
    pub result_raw: TaskResultRaw<T>,
    pub render_time_elapsed_during_the_simulation: Duration,
    /// The time at which we started the simulation, as reported by the used render time [`Time::elapsed`].
    pub started_at_render_time: Duration,
    /// Amount of frames elapsed since the simulation started.
    pub update_frames_elapsed: u32,
}

/// The result of a task to be handled.
#[derive(Default, Component)]
pub struct TaskResults<T: TaskWorkerTrait + Send + Sync> {
    /// The results of the tasks.
    ///
    /// This is a queue because we might be spawning a new task while another has not been processed yet.
    ///
    /// To avoid overwriting the results, we keep them in a queue.
    pub results: VecDeque<TaskResult<T>>,
}

#[derive(Default)]
pub struct BackgroundFixedUpdatePlugin<T: TaskWorkerTrait> {
    pub phantom: std::marker::PhantomData<T>,
}

impl<T: TaskWorkerTrait> Plugin for BackgroundFixedUpdatePlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            bevy::app::prelude::RunFixedMainLoop, // TODO: use a specific schedule for this, à la bevy's FixedMainLoop
            FixedMain::run_schedule::<T>,
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
                .add_systems(handle_task::<T>)
                .set_build_settings(ScheduleBuildSettings {
                    ambiguity_detection: LogLevel::Error,
                    ..default()
                });
        });
        app.edit_schedule(SpawnTask, |schedule| {
            schedule
                .add_systems((extract::<T>, spawn_task::<T>).chain())
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

/// Struct to be able to configure what the task should do.
/// TODO: extract first, then do work.
#[derive(Clone, Component)]
pub struct TaskWorker<T: TaskWorkerTrait> {
    pub worker: T,
}

pub trait TaskWorkerTrait: Clone + Send + Sync + 'static {
    type TaskExtractedData: Clone + Send + Sync + 'static + Component;
    type TaskResultPure: Clone + Send + Sync + 'static;

    fn extract(&self, world: &mut World) -> Self::TaskExtractedData;

    fn work(
        &self,
        data: Self::TaskExtractedData,
        timestep: Duration,
        substep_count: u32,
    ) -> Self::TaskResultPure;

    fn write_back(&self, result: TaskResult<Self>, world: &mut World);
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
    pub fn run_schedule<T: TaskWorkerTrait>(
        world: &mut World,
        mut has_run_at_least_once: Local<bool>,
    ) {
        if !*has_run_at_least_once {
            let _ = world.run_schedule(SpawnTask);
            *has_run_at_least_once = true;
            return;
        }
        world
            .run_system_cached(finish_task_and_store_result::<T>)
            .unwrap();

        // Compute difference between task and render time.
        let clock = world.resource::<Time>().as_generic();
        let mut query = world.query::<(&mut TaskToRenderTime, &Timestep)>();
        let (mut task_to_render_time, timestep) = query.single_mut(world);
        task_to_render_time.diff += clock.delta().as_secs_f64();
        if task_to_render_time.diff < timestep.timestep.as_secs_f64() {
            // Task is too far ahead, we should not read the simulation.
            return;
        }
        let simulated_time = {
            let mut query = world.query::<&TaskResults<T>>();
            let task_result = query.single(world).results.front();
            task_result.map(|task_result| task_result.result_raw.simulated_time)
        };
        let Some(simulated_time) = simulated_time else {
            return;
        };
        let mut query = world.query::<&mut TaskToRenderTime>();
        let mut task_to_render_time = query.single_mut(world);
        task_to_render_time.diff -= simulated_time.as_secs_f64();
        let _ = world.try_schedule_scope(FixedMain, |world, schedule| {
            // Advance simulation.
            schedule.run(world);
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

pub fn extract<T: TaskWorkerTrait>(world: &mut World) {
    let Ok((entity_ctx, worker)) = world
        .query_filtered::<(Entity, &TaskWorker<T>), With<Timestep>>()
        .get_single(&world)
    else {
        info!("No correct entity found.");
        return;
    };

    let extractor = worker.worker.clone();
    let extracted_data = extractor.extract(world);
    world.entity_mut(entity_ctx).insert(extracted_data.clone());
}

/// This system spawns a [`WorkTask`] is none are ongoing.
/// The task simulate computationally intensive work that potentially spans multiple frames/ticks.
///
/// A separate system, [`handle_tasks`], will poll the spawned tasks on subsequent
/// frames/ticks, and use the results to spawn cubes
pub fn spawn_task<T: TaskWorkerTrait>(
    mut commands: Commands,
    q_context: Query<(Entity, &TaskWorker<T>, &Timestep, &T::TaskExtractedData)>,
    virtual_time: Res<Time<Virtual>>,
) {
    let Ok((entity_ctx, worker, timestep, extracted_data)) = q_context.get_single() else {
        return;
    };
    let timestep = timestep.timestep;

    // TODO: tweak this on user side, to allow the simulation to catch up with the render time.
    let substep_count = 1;

    let (sender, recv) = crossbeam_channel::unbounded();

    let transforms_to_move = extracted_data.clone();
    let worker = worker.clone();
    let thread_pool = AsyncComputeTaskPool::get();
    thread_pool
        .spawn(async move {
            let simulated_time = timestep * substep_count;
            profiling::scope!("Rapier physics simulation");
            let transforms_to_move =
                worker
                    .worker
                    .work(transforms_to_move, timestep, substep_count);
            let result = TaskResultRaw::<T> {
                transforms: transforms_to_move,
                simulated_time,
            };
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
pub(crate) fn finish_task_and_store_result<T: TaskWorkerTrait>(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut q_tasks: Query<(Entity, &mut WorkTask<T>, &mut TaskResults<T>)>,
) {
    let Ok((e, mut task, mut results)) = q_tasks.get_single_mut() else {
        return;
    };
    task.update_frames_elapsed += 1;

    let mut handle_result = |task_result_raw: TaskResultRaw<T>| {
        commands.entity(e).remove::<WorkTask<T>>();
        results.results.push_back(TaskResult::<T> {
            result_raw: task_result_raw,
            render_time_elapsed_during_the_simulation: time.elapsed() - task.started_at_render_time,
            started_at_render_time: task.started_at_render_time,
            update_frames_elapsed: task.update_frames_elapsed,
        });
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

pub(crate) fn handle_task<T: TaskWorkerTrait>(world: &mut World) {
    let mut task_results =
        world.query::<(&mut TaskResults<T>, &TaskWorker<T>, &mut TaskToRenderTime)>();

    let mut tasks_to_handle = vec![];
    for (mut results, worker, mut task_to_render) in task_results.iter_mut(world) {
        let Some(task) = results.results.pop_front() else {
            continue;
        };
        task_to_render.last_task_frame_count = task.update_frames_elapsed;
        // Apply transform changes.
        tasks_to_handle.push((worker.clone(), task));
    }

    for (worker, task) in tasks_to_handle {
        worker.worker.write_back(task, world);
    }
}
