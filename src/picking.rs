use bevy::prelude::*;
use bevy_mod_raycast::prelude::{RaycastMesh, RaycastMethod, RaycastSource, RaycastSystem};

use crate::{TransformGizmoSettings, TransformGizmoSystem};

pub type GizmoPickSource = RaycastSource<GizmoRaycastSet>;
pub type PickableGizmo = RaycastMesh<GizmoRaycastSet>;

/// Plugin with all the systems and resources used to raycast against gizmo handles separately from
/// the `bevy_mod_picking` plugin.
pub struct GizmoPickingPlugin;

impl Plugin for GizmoPickingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (
                update_gizmo_raycast_with_cursor,
                bevy_mod_raycast::prelude::build_rays::<GizmoRaycastSet>
                    .in_set(RaycastSystem::BuildRays::<GizmoRaycastSet>),
                bevy_mod_raycast::prelude::update_raycast::<GizmoRaycastSet>
                    .in_set(RaycastSystem::UpdateRaycast::<GizmoRaycastSet>),
            )
                .chain()
                .in_set(TransformGizmoSystem::RaycastSet)
                .run_if(|settings: Res<TransformGizmoSettings>| settings.enabled),
        );
    }
}

#[derive(Reflect, Clone)]
pub struct GizmoRaycastSet;

/// Update the gizmo's raycasting source with the current mouse position.
fn update_gizmo_raycast_with_cursor(
    mut cursor: EventReader<CursorMoved>,
    mut query: Query<&mut GizmoPickSource>,
) {
    for mut pick_source in &mut query {
        // Grab the most recent cursor event if it exists:
        if let Some(cursor_latest) = cursor.read().last() {
            pick_source.cast_method = RaycastMethod::Screenspace(cursor_latest.position);
        }
    }
}
