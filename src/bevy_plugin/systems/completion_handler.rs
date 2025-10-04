use super::super::LoadingTask;
use super::super::resources::MapLoadQueue;
use bevy::{
    prelude::*,
    tasks::{block_on, futures_lite::future},
};

/// System to handle completed async tasks - exactly like the Bevy example
pub fn handle_completed_tasks(
    mut commands: Commands,
    mut queue: ResMut<MapLoadQueue>,
    mut loading_tasks: Query<(Entity, &mut LoadingTask)>,
) {
    let mut completed_cities = Vec::new();

    for (entity, mut loading_task) in &mut loading_tasks {
        // Check if task is complete - exactly like the Bevy example
        if let Some(mut commands_queue) = block_on(future::poll_once(&mut loading_task.task)) {
            // Append the returned command queue to have it execute later
            commands.append(&mut commands_queue);

            // Task is complete, so remove task component from entity
            commands.entity(entity).despawn();

            // Mark for removal from active queue
            completed_cities.push(loading_task.request.city_name.clone());
        }
    }

    // Remove completed tasks from the active queue
    for city in completed_cities {
        queue.active.remove(&city);
    }
}
