use bevy::{prelude::*, window::PresentMode};
use bevy_mod_picking::{DefaultPickingPlugins, PickableBundle};
use irate_transform_gizmo::{GizmoPickSource, GizmoTransformable, TransformGizmoPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: PresentMode::Immediate,
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

fn setup(mut commands: Commands) {
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::new(500., 50.)),
                color: Color::ORANGE_RED,
                ..default()
            },
            transform: Transform::from_xyz(0.0, -100., 0.0),
            ..default()
        },
        PickableBundle::default(),
        GizmoTransformable,
    ));

    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::splat(200.)),
                color: Color::ORANGE,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 25., 0.0),
            ..default()
        },
        PickableBundle::default(),
        GizmoTransformable,
    ));

    commands.spawn((Camera2dBundle::default(), GizmoPickSource::default()));
}
