#![allow(clippy::type_complexity)]

use bevy::asset::load_internal_asset;
use bevy::{prelude::*, render::camera::Projection, transform::TransformSystem};
use bevy_mod_picking::{
    prelude::PickingInteraction,
    selection::{NoDeselect, PickSelection},
};
use gizmo_material::GizmoMaterial;
use mesh::{RotationGizmo, ViewTranslateGizmo};
use normalization::*;

mod gizmo_material;
mod mesh;
pub mod normalization;

pub mod picking;

pub use picking::{GizmoPickSource, PickableGizmo};

#[derive(Resource, Clone, Debug)]
pub struct GizmoSystemsEnabled(pub bool);

pub use normalization::Ui3dNormalization;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum TransformGizmoSystem {
    InputsSet,
    MainSet,
    RaycastSet,
    NormalizeSet,
    UpdateSettings,
    AdjustViewTranslateGizmo,
    Place,
    Hover,
    Grab,
    Drag,
}

#[derive(Debug, Clone, Event)]
pub struct TransformGizmoEvent {
    pub from: GlobalTransform,
    pub to: GlobalTransform,
    pub interaction: TransformGizmoInteraction,
}

#[derive(Component, Default, Clone, Debug)]
pub struct GizmoTransformable;

#[derive(Component, Default, Clone, Debug)]
pub struct InternalGizmoCamera;

#[derive(Resource, Clone, Debug)]
pub struct TransformGizmoSettings {
    pub enabled: bool,
    /// Rotation to apply to the gizmo when it is placed. Used to align the gizmo to a different
    /// coordinate system.
    pub alignment_rotation: Quat,
    pub allow_rotation: bool,
    pub enable_shortcuts: bool,
}

#[derive(Debug, Clone)]
pub struct TransformGizmoPlugin {
    // Rotation to apply to the gizmo when it is placed. Used to align the gizmo to a different
    // coordinate system.
    pub alignment_rotation: Quat,
    pub enable_shortcuts: bool,
}

impl Default for TransformGizmoPlugin {
    fn default() -> Self {
        Self {
            alignment_rotation: Quat::IDENTITY,
            enable_shortcuts: true,
        }
    }
}

impl Plugin for TransformGizmoPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            gizmo_material::GIZMO_SHADER_HANDLE,
            "gizmo_material.wgsl",
            Shader::from_wgsl
        );

        app.insert_resource(TransformGizmoSettings {
            enabled: true,
            alignment_rotation: self.alignment_rotation,
            allow_rotation: true,
            enable_shortcuts: self.enable_shortcuts,
        })
        .insert_resource(GizmoSystemsEnabled(true))
        .add_plugins((
            MaterialPlugin::<GizmoMaterial>::default(),
            picking::GizmoPickingPlugin,
            Ui3dNormalization,
        ))
        .add_event::<TransformGizmoEvent>();

        // Input Set
        app.add_systems(
            PreUpdate,
            update_gizmo_settings
                .in_set(TransformGizmoSystem::UpdateSettings)
                .in_set(TransformGizmoSystem::InputsSet)
                .run_if(|settings: Res<TransformGizmoSettings>| settings.enabled),
        );

        // Main Set
        app.add_systems(
            PostUpdate,
            (
                // drag_gizmo
                //     .in_set(TransformGizmoSystem::Drag)
                //     .before(TransformSystem::TransformPropagate),
                place_gizmo
                    .in_set(TransformGizmoSystem::Place)
                    .after(TransformSystem::TransformPropagate),
                propagate_gizmo_elements,
                adjust_view_translate_gizmo.in_set(TransformGizmoSystem::Drag),
                gizmo_cam_copy_settings.in_set(TransformGizmoSystem::Drag),
            )
                .chain()
                .in_set(TransformGizmoSystem::MainSet)
                .run_if(|settings: Res<TransformGizmoSettings>| settings.enabled),
        );

        app.add_systems(Startup, mesh::build_gizmo)
            .add_systems(PostStartup, place_gizmo);
    }
}

#[derive(Bundle)]
pub struct TransformGizmoBundle {
    gizmo: TransformGizmo,
    picking_interaction: PickingInteraction,
    picking_blocker: NoDeselect,
    transform: Transform,
    global_transform: GlobalTransform,
    visible: Visibility,
    inherited_visibility: InheritedVisibility,
    view_visibility: ViewVisibility,
    normalize: Normalize3d,
}

impl Default for TransformGizmoBundle {
    fn default() -> Self {
        TransformGizmoBundle {
            transform: Transform::from_translation(Vec3::splat(f32::MIN)),
            picking_interaction: PickingInteraction::None,
            picking_blocker: NoDeselect,
            visible: Visibility::Hidden,
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
            gizmo: TransformGizmo::default(),
            global_transform: GlobalTransform::default(),
            normalize: Normalize3d::new(1.5, 150.0),
        }
    }
}

#[derive(Default, PartialEq, Component)]
pub struct TransformGizmo {
    current_interaction: Option<TransformGizmoInteraction>,
    // Point in space where mouse-gizmo interaction started (on mouse down), used to compare how
    // much total dragging has occurred without accumulating error across frames.
    drag_start: Option<Vec3>,
    // Initial transform of the gizmo
    initial_transform: Option<GlobalTransform>,
}

impl TransformGizmo {
    /// Get the gizmo's drag direction.
    pub fn current_interaction(&self) -> Option<TransformGizmoInteraction> {
        self.current_interaction
    }
}

/// Marks the current active gizmo interaction
#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub enum TransformGizmoInteraction {
    TranslateAxis { original: Vec3, axis: Vec3 },
    TranslatePlane { original: Vec3, normal: Vec3 },
    RotateAxis { original: Vec3, axis: Vec3 },
    ScaleAxis { original: Vec3, axis: Vec3 },
}

#[derive(Component)]
struct InitialTransform {
    transform: Transform,
}

/// Places the gizmo in space relative to the selected entity(s).
#[allow(clippy::type_complexity)]
fn place_gizmo(
    plugin_settings: Res<TransformGizmoSettings>,
    mut queries: ParamSet<(
        Query<(&PickSelection, &GlobalTransform), With<GizmoTransformable>>,
        Query<(&mut GlobalTransform, &mut Transform, &mut Visibility), With<TransformGizmo>>,
    )>,
) {
    let selected: Vec<_> = queries
        .p0()
        .iter()
        .filter_map(|(s, t)| s.is_selected.then_some(t.translation()))
        .collect();
    let n_selected = selected.len();
    let transform_sum = selected.iter().fold(Vec3::ZERO, |acc, t| acc + *t);
    let centroid = transform_sum / n_selected as f32;
    // Set the gizmo's position and visibility
    if let Ok((mut g_transform, mut transform, mut visible)) = queries.p1().get_single_mut() {
        let gt = g_transform.compute_transform();
        *g_transform = Transform {
            translation: centroid,
            rotation: plugin_settings.alignment_rotation,
            ..gt
        }
        .into();
        transform.translation = centroid;
        transform.rotation = plugin_settings.alignment_rotation;
        if n_selected > 0 {
            *visible = Visibility::Inherited;
        } else {
            *visible = Visibility::Hidden;
        }
    } else {
        error!("Number of gizmos is != 1");
    }
}

fn propagate_gizmo_elements(
    gizmo: Query<(&GlobalTransform, &Children), With<TransformGizmo>>,
    mut gizmo_parts_query: Query<(&Transform, &mut GlobalTransform), Without<TransformGizmo>>,
) {
    if let Ok((gizmo_pos, gizmo_parts)) = gizmo.get_single() {
        for &entity in gizmo_parts.iter() {
            let (transform, mut g_transform) = gizmo_parts_query.get_mut(entity).unwrap();
            *g_transform = gizmo_pos.mul_transform(*transform);
        }
    }
}

fn update_gizmo_settings(
    plugin_settings: Res<TransformGizmoSettings>,
    mut interactions: Query<&mut TransformGizmoInteraction, Without<ViewTranslateGizmo>>,
    mut rotations: Query<&mut Visibility, With<RotationGizmo>>,
) {
    if !plugin_settings.is_changed() {
        return;
    }
    let rotation = plugin_settings.alignment_rotation;
    for mut interaction in &mut interactions {
        *interaction = match *interaction {
            TransformGizmoInteraction::TranslateAxis { original, axis: _ } => {
                TransformGizmoInteraction::TranslateAxis {
                    original,
                    axis: rotation.mul_vec3(original),
                }
            }
            TransformGizmoInteraction::TranslatePlane {
                original,
                normal: _,
            } => TransformGizmoInteraction::TranslatePlane {
                original,
                normal: rotation.mul_vec3(original),
            },
            TransformGizmoInteraction::RotateAxis { original, axis: _ } => {
                TransformGizmoInteraction::RotateAxis {
                    original,
                    axis: rotation.mul_vec3(original),
                }
            }
            TransformGizmoInteraction::ScaleAxis { original, axis: _ } => {
                TransformGizmoInteraction::ScaleAxis {
                    original,
                    axis: rotation.mul_vec3(original),
                }
            }
        };
    }

    for mut visibility in &mut rotations {
        if plugin_settings.allow_rotation {
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

#[allow(clippy::type_complexity)]
fn adjust_view_translate_gizmo(
    mut gizmo: Query<
        (&mut GlobalTransform, &mut TransformGizmoInteraction),
        (With<ViewTranslateGizmo>, Without<GizmoPickSource>),
    >,
    camera: Query<&Transform, With<GizmoPickSource>>,
) {
    let (mut global_transform, mut interaction) = match gizmo.get_single_mut() {
        Ok(x) => x,
        Err(_) => return,
    };

    let cam_transform = match camera.get_single() {
        Ok(x) => x,
        Err(_) => return,
    };

    let direction = cam_transform.local_z();
    *interaction = TransformGizmoInteraction::TranslatePlane {
        original: Vec3::ZERO,
        normal: direction,
    };
    let rotation = Quat::from_mat3(&Mat3::from_cols(
        direction.cross(cam_transform.local_y()),
        direction,
        cam_transform.local_y(),
    ));
    *global_transform = Transform {
        rotation,
        ..global_transform.compute_transform()
    }
    .into();
}

fn gizmo_cam_copy_settings(
    main_cam: Query<
        (
            Ref<Camera>,
            Ref<GlobalTransform>,
            AnyOf<(Ref<Projection>, Ref<OrthographicProjection>)>,
        ),
        With<GizmoPickSource>,
    >,
    mut gizmo_cam: Query<
        (&mut Camera, &mut GlobalTransform, &mut Projection),
        (With<InternalGizmoCamera>, Without<GizmoPickSource>),
    >,
) {
    let Ok((main_cam, main_cam_pos, (main_proj, main_proj_ortho))) = main_cam.get_single() else {
        error!("No GizmoPickSource found! Insert the GizmoPickSource component onto your primary camera.");
        return;
    };
    let (mut gizmo_cam, mut gizmo_cam_pos, mut proj) = gizmo_cam.single_mut();
    if main_cam_pos.is_changed() {
        *gizmo_cam_pos = *main_cam_pos;
    }
    if main_cam.is_changed() {
        *gizmo_cam = main_cam.clone();
        gizmo_cam.order += 10;
    }
    if let Some(main_proj) = main_proj {
        if main_proj.is_changed() {
            *proj = main_proj.clone();
        }
    } else if let Some(main_proj_ortho) = main_proj_ortho {
        if main_proj_ortho.is_changed() {
            *proj = Projection::Orthographic(main_proj_ortho.clone());
        }
    }
}
