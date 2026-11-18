//! Packaged convex MeshCollider vertices in prefab-local space, including child rotations.
use crate::math::V;
use std::sync::OnceLock;
struct Hull {
    kind: u8,
    size: [f32; 3],
    points: Vec<[f32; 3]>,
}
static HULLS: OnceLock<Vec<Hull>> = OnceLock::new();
pub fn points(kind: u8, half: V) -> Option<Vec<[f32; 3]>> {
    let hulls = HULLS.get_or_init(|| {
        include_str!("../assets/physics/colliders.txt")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .map(|l| {
                let n: Vec<f32> = l
                    .split_whitespace()
                    .map(|x| x.parse().expect("invalid recovered collider"))
                    .collect();
                let points: Vec<_> = n[5..].chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect();
                assert_eq!(points.len(), n[4] as usize);
                Hull {
                    kind: n[0] as u8,
                    size: [n[1], n[2], n[3]],
                    points,
                }
            })
            .collect()
    });
    let size = [half.x * 2., half.y * 2., half.z * 2.];
    let hull = hulls.iter().filter(|h| h.kind == kind).min_by(|a, b| {
        let score = |h: &Hull| {
            size.iter()
                .zip(h.size)
                .map(|(a, b)| (a / b).ln().abs())
                .sum::<f32>()
        };
        score(a).total_cmp(&score(b))
    })?;
    Some(
        hull.points
            .iter()
            .map(|p| std::array::from_fn(|i| p[i] * size[i] / hull.size[i]))
            .collect(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sideways_cans_use_sideways_collision_geometry() {
        let p = points(1, V::new(1.5, 0.5, 0.5)).unwrap();
        let extents: Vec<_> = (0..3)
            .map(|i| p.iter().map(|p| p[i].abs()).fold(0., f32::max))
            .collect();
        assert!(extents[0] > 1.45 && extents[0] < 1.51);
        assert!(extents[1] < 0.501 && extents[2] < 0.501);
    }
    #[test]
    fn recovered_collision_vertices_are_valid() {
        for kind in [1, 2, 3, 11, 12, 13, 14, 15, 16, 21, 22, 41, 42, 51, 61, 62] {
            let p = points(kind, V::new(0.5, 0.5, 0.5)).unwrap();
            assert!(p.len() >= 4);
            assert!(
                p.iter().flatten().all(|x| x.is_finite() && x.abs() < 0.6),
                "bad bounds for {kind}"
            );
        }
    }
}
