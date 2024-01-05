use crate::{
    gizmo_material::GizmoMaterial, GizmoPickSource, InitialTransform, InternalGizmoCamera,
    PickableGizmo, TransformGizmo, TransformGizmoBundle, TransformGizmoInteraction,
};
use bevy::{
    core_pipeline::{clear_color::ClearColorConfig, core_3d::Camera3dDepthLoadOp},
    pbr::NotShadowCaster,
    prelude::*,
    render::view::RenderLayers,
};
use bevy_mod_picking::{
    events::{Drag, DragEnd, DragStart, Move, Out, Pointer},
    prelude::{Listener, On},
    selection::{NoDeselect, PickSelection},
};
use bevy_mod_raycast::{prelude::NoBackfaceCulling, primitives::Primitive3d};

mod cone;
mod truncated_torus;

#[derive(Component)]
pub struct RotationGizmo;

#[derive(Component)]
pub struct ViewTranslateGizmo;

fn on_drag_start(
    event: Listener<Pointer<DragStart>>,
    selected_items_query: Query<(&PickSelection, &GlobalTransform, Entity)>,
    parents: Query<(&TransformGizmoInteraction, &Parent)>,
    mut gizmo: Query<(&GlobalTransform, &mut TransformGizmo)>,
    mut commands: Commands,
) {
    // Dragging has started, store the initial position of all selected meshes
    for (selection, transform, entity) in &selected_items_query {
        if selection.is_selected {
            commands.entity(entity).insert(InitialTransform {
                transform: transform.compute_transform(),
            });
        }
    }

    let Ok((t, parent)) = parents.get(event.target) else {
        return;
    };

    let (transform, mut gizmo) = gizmo.get_mut(parent.get()).unwrap();

    gizmo.initial_transform = Some(*transform);
    gizmo.current_interaction = Some(*t);
}

fn on_drag_end(
    event: Listener<Pointer<DragEnd>>,
    selected_items_query: Query<Entity, With<InitialTransform>>,
    parents: Query<&Parent>,
    mut gizmo: Query<&mut TransformGizmo>,
    mut commands: Commands,
) {
    // Dragging has started, store the initial position of all selected meshes
    for entity in &selected_items_query {
        commands.entity(entity).remove::<InitialTransform>();
    }

    let Ok(parent) = parents.get(event.target) else {
        return;
    };

    let mut gizmo = gizmo.get_mut(parent.get()).unwrap();

    gizmo.initial_transform = None;
    gizmo.drag_start = None;
    gizmo.current_interaction = None;
}

fn on_drag(
    event: Listener<Pointer<Drag>>,
    parents: Query<&Parent>,
    mut gizmo: Query<(&GlobalTransform, &mut TransformGizmo)>,
    pick_cam: Query<&GizmoPickSource>,

    mut transform_query: Query<
        (
            &PickSelection,
            Option<&Parent>,
            &mut Transform,
            &InitialTransform,
        ),
        Without<TransformGizmo>,
    >,
    // cameras: Query<(&Camera, &GlobalTransform)>,
    global_transforms: Query<&GlobalTransform>,
) {
    let Ok(picking_camera) = pick_cam.get_single() else {
        return; // Not exactly one picking camera.
    };
    let Some(picking_ray) = picking_camera.get_ray() else {
        return; // Picking camera does not have a ray.
    };
    // let (camera, camera_transform) = cameras.get(event.).unwrap();

    let (gizmo_transform, mut gizmo) = gizmo
        .get_mut(parents.get(event.target).unwrap().get())
        .unwrap();

    let Some(gizmo_origin) = gizmo.initial_transform.map(|t| t.translation()) else {
        return;
    };

    let selected_iter = transform_query
        .iter_mut()
        .filter(|(s, ..)| s.is_selected)
        .map(|(_, parent, local_transform, initial_global_transform)| {
            let parent_global_transform = parent
                .and_then(|parent| global_transforms.get(parent.get()).ok())
                .unwrap_or(&GlobalTransform::IDENTITY);
            let parent_mat = parent_global_transform.compute_matrix();
            let inverse_parent = parent_mat.inverse();
            (inverse_parent, local_transform, initial_global_transform)
        });

    if let Some(interaction) = gizmo.current_interaction {
        if gizmo.initial_transform.is_none() {
            gizmo.initial_transform = Some(*gizmo_transform);
        }
        match interaction {
            TransformGizmoInteraction::TranslateAxis { original: _, axis } => {
                let vertical_vector = picking_ray.direction().cross(axis).normalize();
                let plane_normal = axis.cross(vertical_vector).normalize();
                let plane_origin = gizmo_origin;
                let cursor_plane_intersection = if let Some(intersection) = picking_camera
                    .intersect_primitive(Primitive3d::Plane {
                        normal: plane_normal,
                        point: plane_origin,
                    }) {
                    intersection.position()
                } else {
                    return;
                };
                let cursor_vector: Vec3 = cursor_plane_intersection - plane_origin;
                let Some(cursor_projected_onto_handle) = gizmo.drag_start else {
                    let handle_vector = axis;
                    let cursor_projected_onto_handle =
                        cursor_vector.dot(handle_vector.normalize()) * handle_vector.normalize();
                    gizmo.drag_start = Some(cursor_projected_onto_handle + plane_origin);
                    return;
                };
                let selected_handle_vec = cursor_projected_onto_handle - plane_origin;
                let new_handle_vec = cursor_vector.dot(selected_handle_vec.normalize())
                    * selected_handle_vec.normalize();
                let translation = new_handle_vec - selected_handle_vec;
                selected_iter.for_each(
                    |(inverse_parent, mut local_transform, initial_global_transform)| {
                        let new_transform = Transform {
                            translation: initial_global_transform.transform.translation
                                + translation,
                            rotation: initial_global_transform.transform.rotation,
                            scale: initial_global_transform.transform.scale,
                        };
                        let local = inverse_parent * new_transform.compute_matrix();
                        local_transform.set_if_neq(Transform::from_matrix(local));
                    },
                );
            }
            TransformGizmoInteraction::TranslatePlane { normal, .. } => {
                let plane_origin = gizmo_origin;
                let cursor_plane_intersection = if let Some(intersection) = picking_camera
                    .intersect_primitive(Primitive3d::Plane {
                        normal,
                        point: plane_origin,
                    }) {
                    intersection.position()
                } else {
                    return;
                };
                let Some(drag_start) = gizmo.drag_start else {
                    gizmo.drag_start = Some(cursor_plane_intersection);
                    return; // We just started dragging, no transformation is needed yet, exit early.
                };
                selected_iter.for_each(
                    |(inverse_parent, mut local_transform, initial_transform)| {
                        let new_transform = Transform {
                            translation: initial_transform.transform.translation
                                + cursor_plane_intersection
                                - drag_start,
                            rotation: initial_transform.transform.rotation,
                            scale: initial_transform.transform.scale,
                        };
                        let local = inverse_parent * new_transform.compute_matrix();
                        local_transform.set_if_neq(Transform::from_matrix(local));
                    },
                );
            }
            TransformGizmoInteraction::RotateAxis { original: _, axis } => {
                let rotation_plane = Primitive3d::Plane {
                    normal: axis.normalize(),
                    point: gizmo_origin,
                };
                let cursor_plane_intersection = if let Some(intersection) =
                    picking_camera.intersect_primitive(rotation_plane)
                {
                    intersection.position()
                } else {
                    return;
                };
                let cursor_vector = (cursor_plane_intersection - gizmo_origin).normalize();
                let Some(drag_start) = gizmo.drag_start else {
                    gizmo.drag_start = Some(cursor_vector);
                    return; // We just started dragging, no transformation is needed yet, exit early.
                };
                let dot = drag_start.dot(cursor_vector);
                let det = axis.dot(drag_start.cross(cursor_vector));
                let angle = det.atan2(dot);
                let rotation = Quat::from_axis_angle(axis, angle);
                selected_iter.for_each(
                    |(inverse_parent, mut local_transform, initial_transform)| {
                        let mut new_transform = initial_transform.transform;
                        new_transform.rotate_around(gizmo_origin, rotation);
                        let local = inverse_parent * new_transform.compute_matrix();
                        local_transform.set_if_neq(Transform::from_matrix(local));
                    },
                );
            }
            TransformGizmoInteraction::ScaleAxis {
                original: _,
                axis: _,
            } => (),
        }
    }
}

/// Startup system that builds the procedural mesh and materials of the gizmo.
pub fn build_gizmo(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
) {
    let axis_length = 1.3;
    let arc_radius = 1.;
    let plane_size = axis_length * 0.35;
    let plane_offset = plane_size / 2.;
    // Define gizmo meshes
    let arrow_tail_mesh = meshes.add(Mesh::from(shape::Cylinder {
        radius: 0.04,
        height: axis_length,
        ..Default::default()
    }));
    let cone_mesh = meshes.add(Mesh::from(cone::Cone {
        height: 0.25,
        radius: 0.10,
        ..Default::default()
    }));
    let plane_mesh = meshes.add(Mesh::from(shape::Plane::from_size(plane_size)));
    let sphere_mesh = meshes.add(
        Mesh::try_from(shape::Icosphere {
            radius: 0.25,
            subdivisions: 3,
        })
        .unwrap(),
    );
    let rotation_mesh = meshes.add(Mesh::from(truncated_torus::TruncatedTorus {
        radius: arc_radius,
        ring_radius: 0.04,
        ..Default::default()
    }));
    //let cube_mesh = meshes.add(Mesh::from(shape::Cube { size: 0.15 }));
    // Define gizmo materials
    let s = 0.8;
    let l = 0.55;
    let l_selected = 0.7;

    let x = Color::hsl(0.0, s, l);
    let y = Color::hsl(120.0, s, l);
    let z = Color::hsl(240.0, s, l);

    let plane_alpha = 0.5;

    let x_translation = materials.add(x.into());
    let x_translation_plane = materials.add(x.with_a(plane_alpha).into());
    let x_rotation = materials.add(x.into());

    let y_translation = materials.add(y.into());
    let y_translation_plane = materials.add(y.with_a(plane_alpha).into());
    let y_rotation = materials.add(y.into());

    let z_translation = materials.add(z.into());
    let z_translation_plane = materials.add(z.with_a(plane_alpha).into());
    let z_rotation = materials.add(z.into());

    let v = materials.add(GizmoMaterial::from(Color::hsl(0., 0.0, l)));

    // Build the gizmo using the variables above.
    commands
        .spawn((
            TransformGizmoBundle::default(),
            On::<Pointer<Move>>::run(
                move |event: Listener<Pointer<Move>>,
                      mut assets: ResMut<Assets<GizmoMaterial>>,
                      handles: Query<&Handle<GizmoMaterial>>| {
                    let Ok(handle) = handles.get(event.target) else {
                        return;
                    };
                    assets.get_mut(handle).unwrap().color.set_l(l_selected);
                },
            ),
            On::<Pointer<Out>>::run(
                move |event: Listener<Pointer<Out>>,
                      mut assets: ResMut<Assets<GizmoMaterial>>,
                      handles: Query<&Handle<GizmoMaterial>>| {
                    let Ok(handle) = handles.get(event.target) else {
                        return;
                    };
                    assets.get_mut(handle).unwrap().color.set_l(l);
                },
            ),
            On::<Pointer<DragStart>>::run(on_drag_start),
            On::<Pointer<DragEnd>>::run(on_drag_end),
            On::<Pointer<Drag>>::run(on_drag),
        ))
        .with_children(|parent| {
            // Translation Axes
            parent.spawn((
                MaterialMeshBundle {
                    mesh: arrow_tail_mesh.clone(),
                    material: x_translation.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_z(std::f32::consts::PI / 2.0),
                        Vec3::new(axis_length / 2.0, 0.0, 0.0),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslateAxis {
                    original: Vec3::X,
                    axis: Vec3::X,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: arrow_tail_mesh.clone(),
                    material: y_translation.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_y(std::f32::consts::PI / 2.0),
                        Vec3::new(0.0, axis_length / 2.0, 0.0),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslateAxis {
                    original: Vec3::Y,
                    axis: Vec3::Y,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: arrow_tail_mesh,
                    material: z_translation.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_x(std::f32::consts::PI / 2.0),
                        Vec3::new(0.0, 0.0, axis_length / 2.0),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslateAxis {
                    original: Vec3::Z,
                    axis: Vec3::Z,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));

            // Translation Handles
            parent.spawn((
                MaterialMeshBundle {
                    mesh: cone_mesh.clone(),
                    material: x_translation.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_z(std::f32::consts::PI / -2.0),
                        Vec3::new(axis_length, 0.0, 0.0),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslateAxis {
                    original: Vec3::X,
                    axis: Vec3::X,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: plane_mesh.clone(),
                    material: x_translation_plane.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_z(std::f32::consts::PI / -2.0),
                        Vec3::new(0., plane_offset, plane_offset),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslatePlane {
                    original: Vec3::X,
                    normal: Vec3::X,
                },
                NoBackfaceCulling,
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: cone_mesh.clone(),
                    material: y_translation.clone(),
                    transform: Transform::from_translation(Vec3::new(0.0, axis_length, 0.0)),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslateAxis {
                    original: Vec3::Y,
                    axis: Vec3::Y,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: plane_mesh.clone(),
                    material: y_translation_plane.clone(),
                    transform: Transform::from_translation(Vec3::new(
                        plane_offset,
                        0.0,
                        plane_offset,
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslatePlane {
                    original: Vec3::Y,
                    normal: Vec3::Y,
                },
                NoBackfaceCulling,
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: cone_mesh.clone(),
                    material: z_translation.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_x(std::f32::consts::PI / 2.0),
                        Vec3::new(0.0, 0.0, axis_length),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslateAxis {
                    original: Vec3::Z,
                    axis: Vec3::Z,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: plane_mesh.clone(),
                    material: z_translation_plane.clone(),
                    transform: Transform::from_matrix(Mat4::from_rotation_translation(
                        Quat::from_rotation_x(std::f32::consts::PI / 2.0),
                        Vec3::new(plane_offset, plane_offset, 0.0),
                    )),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslatePlane {
                    original: Vec3::Z,
                    normal: Vec3::Z,
                },
                NoBackfaceCulling,
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));

            parent.spawn((
                MaterialMeshBundle {
                    mesh: sphere_mesh.clone(),
                    material: v.clone(),
                    ..Default::default()
                },
                PickableGizmo::default(),
                TransformGizmoInteraction::TranslatePlane {
                    original: Vec3::ZERO,
                    normal: Vec3::Z,
                },
                ViewTranslateGizmo,
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));

            // Rotation Arcs
            parent.spawn((
                MaterialMeshBundle {
                    mesh: rotation_mesh.clone(),
                    material: x_rotation.clone(),
                    transform: Transform::from_rotation(Quat::from_axis_angle(
                        Vec3::Z,
                        f32::to_radians(90.0),
                    )),
                    ..Default::default()
                },
                RotationGizmo,
                PickableGizmo::default(),
                TransformGizmoInteraction::RotateAxis {
                    original: Vec3::X,
                    axis: Vec3::X,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: rotation_mesh.clone(),
                    material: y_rotation.clone(),
                    ..Default::default()
                },
                RotationGizmo,
                PickableGizmo::default(),
                TransformGizmoInteraction::RotateAxis {
                    original: Vec3::Y,
                    axis: Vec3::Y,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
            parent.spawn((
                MaterialMeshBundle {
                    mesh: rotation_mesh.clone(),
                    material: z_rotation.clone(),
                    transform: Transform::from_rotation(
                        Quat::from_axis_angle(Vec3::Z, f32::to_radians(90.0))
                            * Quat::from_axis_angle(Vec3::X, f32::to_radians(90.0)),
                    ),
                    ..Default::default()
                },
                RotationGizmo,
                PickableGizmo::default(),
                TransformGizmoInteraction::RotateAxis {
                    original: Vec3::Z,
                    axis: Vec3::Z,
                },
                NotShadowCaster,
                RenderLayers::layer(12),
                NoDeselect,
            ));
        });

    commands.spawn((
        Camera3dBundle {
            camera_3d: Camera3d {
                clear_color: ClearColorConfig::None,
                depth_load_op: Camera3dDepthLoadOp::Clear(0.),
                ..default()
            },
            ..Default::default()
        },
        InternalGizmoCamera,
        RenderLayers::layer(12),
    ));
}
