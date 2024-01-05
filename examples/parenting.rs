use bevy::{prelude::*, window::PresentMode};
use bevy_mod_picking::{DefaultPickingPlugins, PickableBundle};
use irate_transform_gizmo::{GizmoPickSource, GizmoTransformable, TransformGizmoPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
            DefaultPickingPlugins,
            TransformGizmoPlugin::default(),
        ))
        .add_systems(Startup, setup)
        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane {
                size: 5.0,
                ..default()
            })),
            material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
            transform: Transform::from_xyz(0.0, -0.5, 0.0),
            ..default()
        },
        PickableBundle::default(),
        GizmoTransformable,
    ));

    // cube
    commands
        .spawn((
            PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
                material: materials.add(Color::ORANGE_RED.into()),
                transform: Transform::from_xyz(-1.0, 0.0, 1.0),
                ..default()
            },
            PickableBundle::default(),
            GizmoTransformable,
        ))
        .with_children(|commands| {
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
                    material: materials.add(Color::ORANGE.into()),
                    transform: Transform::from_xyz(1.0, 0.0, 0.0),
                    ..default()
                },
                PickableBundle::default(),
                GizmoTransformable,
            ));
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
                    material: materials.add(Color::ORANGE.into()),
                    transform: Transform::from_xyz(1.0, 1.0, 0.0),
                    ..default()
                },
                PickableBundle::default(),
                GizmoTransformable,
            ));
        });

    // light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::default().looking_to(Vec3::NEG_ONE, Vec3::Y),
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        ..default()
    });
    // camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        GizmoPickSource::default(),
    ));
}
