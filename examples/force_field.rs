//! Force field example.
//!
//! This example demonstrates how to use the `ConformToSphereModifier` to
//! simulate attraction and repulsion forces. The example is interactif; left
//! clicking spawns particles that are repulsed by one point and attracted by
//! another. The attractor also conforms the particles that are close to a
//! sphere around it.
//!
//! The example also demonstrates the `KillAabbModifier` and
//! `KillSphereModifier`: a green "allow" box to which particles are confined,
//! and a red "forbid" sphere killing all particles entering it.
//!
//! Note: Some particles may _appear_ to penetrate the red "forbid" sphere due
//! to the projection on screen; however those particles are actually at a
//! different depth, in front or behind the sphere.

use bevy::{core_pipeline::tonemapping::Tonemapping, prelude::*};
use bevy_hanabi::prelude::*;

mod utils;
use utils::*;

const DEMO_DESC: &str = include_str!("force_field.txt");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = utils::DemoApp::new("force_field")
        .with_desc(DEMO_DESC)
        .with_desc_position(DescPosition::BottomRow)
        .build();

    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn_on_click, move_repulsor))
        .run();

    app.run().into_result()
}

const BALL_RADIUS: f32 = 0.05;

#[derive(Component)]
struct RepulsorMarker(pub bool);

const ATTRACTOR_POS: Vec3 = Vec3::new(0.01, 0.0, 0.0);
const REPULSOR_POS: Vec3 = Vec3::new(0.3, 0.5, 0.0);

fn setup(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut projection = OrthographicProjection::default_3d();
    projection.scaling_mode = bevy::render::camera::ScalingMode::FixedVertical {
        viewport_height: 5.,
    };
    commands.spawn((
        Transform::from_translation(Vec3::Z * 10.),
        Camera3d::default(),
        Projection::Orthographic(projection),
        Tonemapping::None,
    ));

    commands.spawn((PointLight::default(), Transform::from_xyz(4.0, 5.0, 4.0)));
    commands.spawn((PointLight::default(), Transform::from_xyz(4.0, -5.0, -4.0)));

    // Visual marker for attractor sphere
    commands.spawn((
        Transform::from_translation(ATTRACTOR_POS),
        Mesh3d(meshes.add(Mesh::from(Sphere {
            radius: BALL_RADIUS * 2.0,
        }))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: utils::COLOR_YELLOW,
            unlit: false,
            ..Default::default()
        })),
    ));

    // Visual marker for repulsor sphere
    commands.spawn((
        Transform::from_translation(REPULSOR_POS),
        Mesh3d(meshes.add(Mesh::from(Sphere {
            radius: BALL_RADIUS * 1.0,
        }))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: utils::COLOR_PURPLE,
            unlit: false,
            ..Default::default()
        })),
        RepulsorMarker(true),
    ));

    // "allow" box
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(6., 4., 6.))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::linear_rgba(0., 0.7, 0., 0.05),
            unlit: true,
            alpha_mode: bevy::prelude::AlphaMode::Blend,
            ..Default::default()
        })),
    ));

    // "forbid" sphere
    commands.spawn((
        Transform::from_translation(Vec3::new(-2., 1., 0.1)),
        Mesh3d(meshes.add(Sphere { radius: 0.6 })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::linear_rgba(0.7, 0., 0., 0.2),
            unlit: true,
            alpha_mode: bevy::prelude::AlphaMode::Blend,
            ..Default::default()
        })),
    ));

    let mut gradient = Gradient::new();
    gradient.add_key(0.0, Vec4::new(0.0, 1.0, 1.0, 1.0));
    gradient.add_key(1.0, Vec4::new(0.0, 1.0, 1.0, 0.0));

    // Each mouse click spawns a burst of 30 particles, once.
    let spawner = SpawnerSettings::once(30.0.into())
        // Prevent the spawner from immediately emitting particles on component
        // spawning. This allows controling emitting with a mouse button.
        .with_emit_on_start(false);

    let writer = ExprWriter::new();

    let age = writer.lit(0.).expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, age);

    let lifetime = writer.lit(10.).expr();
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Define the AABB within which particles are confined. Any particle attempting
    // to leave gets killed.
    let center = writer.lit(Vec3::ZERO).expr();
    let half_size = writer.lit(Vec3::new(3., 2., 3.)).expr();
    let allow_zone = KillAabbModifier::new(center, half_size);

    // Define the sphere into which particles cannot enter. Any particle attempting
    // to enter gets killed.
    let center = writer.lit(Vec3::new(-2., 1., 0.)).expr();
    let radius = writer.lit(0.6);
    let sqr_radius = (radius.clone() * radius).expr();
    let deny_zone = KillSphereModifier::new(center, sqr_radius).with_kill_inside(true);

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(BALL_RADIUS).expr(),
        dimension: ShapeDimension::Surface,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: (writer.rand(ScalarType::Float) * writer.lit(0.2) + writer.lit(0.1)).expr(),
    };

    // Sphere repulsor pushing particles away. The acceleration is negative to
    // repulse partices.
    let repulsor_accel = writer.add_property("repulsor_accel", Value::Scalar((-15.0).into()));
    let repulsor_position =
        writer.add_property("repulsor_position", Value::Vector(REPULSOR_POS.into()));
    let repulsor_accel = writer.prop(repulsor_accel);
    let update_repulsor = ConformToSphereModifier {
        origin: writer.prop(repulsor_position).expr(),
        radius: writer.lit(BALL_RADIUS).expr(),
        influence_dist: writer.lit(BALL_RADIUS * 10.).expr(),
        attraction_accel: repulsor_accel.expr(),
        max_attraction_speed: writer.lit(10.).expr(),
        sticky_factor: None,
        shell_half_thickness: None,
    };

    // Sphere attractor with conforming. The particles are attracted to the sphere
    // surface, and tend to "stick" onto it.
    let attraction_accel = writer.add_property("attraction_accel", Value::Scalar(20.0.into()));
    let max_attraction_speed =
        writer.add_property("max_attraction_speed", Value::Scalar(5.0.into()));
    let sticky_factor = writer.add_property("sticky_factor", Value::Scalar(2.0.into()));
    let shell_half_thickness =
        writer.add_property("shell_half_thickness", Value::Scalar(0.1.into()));
    let update_attractor = ConformToSphereModifier {
        origin: writer.lit(ATTRACTOR_POS).expr(),
        radius: writer.lit(BALL_RADIUS * 6.).expr(),
        influence_dist: writer.lit(BALL_RADIUS * 100.).expr(),
        attraction_accel: writer.prop(attraction_accel).expr(),
        max_attraction_speed: writer.prop(max_attraction_speed).expr(),
        sticky_factor: Some(writer.prop(sticky_factor).expr()),
        shell_half_thickness: Some(writer.prop(shell_half_thickness).expr()),
    };

    // Force field effects
    let effect = effects.add(
        EffectAsset::new(32768, spawner, writer.finish())
            .with_name("force_field")
            .init(init_pos)
            .init(init_vel)
            .init(init_age)
            .init(init_lifetime)
            .update(update_attractor)
            .update(update_repulsor)
            .update(allow_zone)
            .update(deny_zone)
            .render(SizeOverLifetimeModifier {
                gradient: Gradient::constant(Vec3::splat(0.05)),
                screen_space_size: false,
            })
            .render(ColorOverLifetimeModifier::new(gradient)),
    );

    commands.spawn((ParticleEffect::new(effect), EffectProperties::default()));
}

fn spawn_on_click(
    mut q_effect: Query<(&mut EffectSpawner, &mut Transform), Without<Projection>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    // Note: On first frame where the effect spawns, EffectSpawner is spawned during
    // CoreSet::PostUpdate, so will not be available yet. Ignore for a frame if
    // so.
    let Ok((mut effect_spawner, mut effect_transform)) = q_effect.single_mut() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    if let Ok(window) = window.single() {
        if let Some(mouse_pos) = window.cursor_position() {
            if mouse_button_input.just_pressed(MouseButton::Left) {
                let ray = camera
                    .viewport_to_world(camera_transform, mouse_pos)
                    .unwrap();
                let spawning_pos = Vec3::new(ray.origin.x, ray.origin.y, 0.);

                effect_transform.translation = spawning_pos;

                // Spawn a single burst of particles
                effect_spawner.reset();
            }
        }
    }
}

fn move_repulsor(
    time: Res<Time>,
    mut q_properties: Query<&mut EffectProperties>,
    mut q_marker: Query<(&mut Transform, &RepulsorMarker)>,
) {
    // Calculate new repulsor position
    let time = time.elapsed_secs();
    let mut pos = REPULSOR_POS + Vec3::Y * (time / 2.).sin();

    // Move the entity so we can visualize the change
    if let Ok((mut transform, marker)) = q_marker.single_mut() {
        if !marker.0 {
            // "hide"/"disable" by sending so far away it has no actual effect and is
            // invisible
            pos.x += 1e9;
        }
        transform.translation = pos;
    }

    // Assign new position to property
    if let Ok(mut properties) = q_properties.single_mut() {
        properties.set("repulsor_position", pos.into());
    }
}
