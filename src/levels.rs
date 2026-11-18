use crate::math::V;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::fs;

const LEVEL_DATA: &str = "assets/levels.json";

#[derive(Clone)]
pub struct Spec {
    pub p: V,
    pub h: V,
    pub t: u8,
    pub q: [f32; 4],
    pub motion: [f32; 12],
    pub support: usize,
}

pub struct Level {
    pub shots: i32,
    pub number: usize,
    pub stage: usize,
    pub stage_count: usize,
    pub world_y_offset: f32,
    pub boxes: Vec<Spec>,
    pub tables: Vec<Spec>,
}

#[derive(Deserialize)]
struct Stage {
    number: usize,
    stage: usize,
    stage_count: usize,
    shots: i32,
    world_y_offset: f32,
    platforms: Vec<Platform>,
    objects: Vec<Object>,
}

#[derive(Deserialize)]
struct Platform {
    position: [f32; 3],
    half_extents: [f32; 3],
    rotation: [f32; 4],
    #[serde(default)]
    motion: [f32; 12],
}

#[derive(Deserialize)]
struct Object {
    position: [f32; 3],
    half_extents: [f32; 3],
    rotation: [f32; 4],
    #[serde(rename = "type")]
    kind: u8,
    #[serde(default)]
    support: usize,
}

impl Spec {
    fn from_pose(position: [f32; 3], half: [f32; 3], rotation: [f32; 4]) -> Result<Self> {
        ensure!(
            position.iter().chain(&rotation).all(|v| v.is_finite())
                && half.iter().all(|v| v.is_finite() && *v > 0.),
            "invalid position, half extents, or rotation"
        );

        // The data uses Unity coordinates. Reflect X to match the exported meshes.
        let [x, y, z] = position;
        let [qx, qy, qz, qw] = rotation;
        Ok(Self {
            p: V::new(-x, y, z),
            h: V::new(half[0], half[1], half[2]),
            t: 0,
            q: [qx, -qy, -qz, qw],
            motion: [0.; 12],
            support: 0,
        })
    }
}

impl Stage {
    fn into_level(self) -> Result<Level> {
        ensure!(
            self.number > 0
                && self.stage < self.stage_count
                && self.shots > 0
                && self.world_y_offset.is_finite()
                && !self.platforms.is_empty()
                && !self.objects.is_empty(),
            "invalid stage metadata or empty layout"
        );

        let mut tables = Vec::with_capacity(self.platforms.len());
        for (index, platform) in self.platforms.into_iter().enumerate() {
            let mut spec =
                Spec::from_pose(platform.position, platform.half_extents, platform.rotation)
                    .with_context(|| format!("platform {index}"))?;
            let mut motion = platform.motion;
            ensure!(
                motion.iter().all(|v| v.is_finite()),
                "invalid motion for platform {index}"
            );
            // Reflect the rotation speed and horizontal movement direction with X.
            motion[1] = -motion[1];
            motion[5] = if motion[5] == 0. { 1. } else { 0. };
            spec.motion = motion;
            tables.push(spec);
        }

        let mut boxes = Vec::with_capacity(self.objects.len());
        for (index, object) in self.objects.into_iter().enumerate() {
            let mut spec = Spec::from_pose(object.position, object.half_extents, object.rotation)
                .with_context(|| format!("object {index}"))?;
            ensure!(
                object.support < tables.len(),
                "invalid supporting platform for object {index}"
            );
            spec.t = object.kind;
            spec.support = object.support;
            boxes.push(spec);
        }

        Ok(Level {
            shots: self.shots,
            number: self.number,
            stage: self.stage,
            stage_count: self.stage_count,
            world_y_offset: self.world_y_offset,
            boxes,
            tables,
        })
    }
}

pub fn load() -> Result<Vec<Level>> {
    let data = fs::read(LEVEL_DATA).with_context(|| format!("cannot read {LEVEL_DATA}"))?;
    let stages: Vec<Stage> =
        serde_json::from_slice(&data).with_context(|| format!("invalid JSON in {LEVEL_DATA}"))?;
    ensure!(!stages.is_empty(), "{LEVEL_DATA} contains no stages");
    stages
        .into_iter()
        .map(|stage| {
            let label = (stage.number, stage.stage);
            stage
                .into_level()
                .with_context(|| format!("{LEVEL_DATA}: level {} stage index {}", label.0, label.1))
        })
        .collect()
}
