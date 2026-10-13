use std::collections::BTreeMap;

use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
    path::{Path, math::Point, math::point, path::Builder},
};

use super::{TextError, TextVertex};
use crate::math::Vec3;

struct Outline {
    builder: Builder,
    open: bool,
}

pub(super) fn extrude(
    face: &ttf_parser::Face<'_>,
    glyph_index: u16,
    scale: f32,
    depth: f32,
    offset: &Vec3,
    color: [f32; 4],
) -> Result<Vec<TextVertex>, TextError> {
    let mut outline = Outline::new();
    if face
        .outline_glyph(ttf_parser::GlyphId(glyph_index), &mut outline)
        .is_none()
    {
        return Ok(Vec::new());
    }

    extrude_path(
        &outline.finish(),
        face.units_per_em() as f32 / 512.0,
        scale,
        depth,
        offset,
        color,
    )
}

fn extrude_path(
    path: &Path,
    tolerance: f32,
    scale: f32,
    depth: f32,
    offset: &Vec3,
    color: [f32; 4],
) -> Result<Vec<TextVertex>, TextError> {
    let mut geometry = VertexBuffers::<Point, u32>::new();
    FillTessellator::new().tessellate_path(
        path,
        &FillOptions::default()
            .with_fill_rule(FillRule::NonZero)
            .with_tolerance(tolerance),
        &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex<'_>| vertex.position()),
    )?;

    let vertex = |index: u32, z: f32, normal: &Vec3| {
        let point = geometry.vertices[index as usize];
        [
            offset.x + point.x * scale,
            offset.y + point.y * scale,
            offset.z + z,
            normal.x,
            normal.y,
            normal.z,
            0.0,
            0.0,
            color[0],
            color[1],
            color[2],
            color[3],
        ]
    };
    let mut vertices = Vec::with_capacity(geometry.indices.len() * 2);
    let mut edges = BTreeMap::new();
    for &[a, mut b, mut c] in geometry.indices.as_chunks::<3>().0 {
        let origin = geometry.vertices[a as usize];
        let ab = geometry.vertices[b as usize] - origin;
        let ac = geometry.vertices[c as usize] - origin;
        let area = ab.cross(ac);
        if area == 0.0 {
            continue;
        }
        if area > 0.0 {
            std::mem::swap(&mut b, &mut c);
        }

        vertices.extend_from_slice(&[
            vertex(a, 0.0, &Vec3::NEG_Z),
            vertex(b, 0.0, &Vec3::NEG_Z),
            vertex(c, 0.0, &Vec3::NEG_Z),
            vertex(a, depth, &Vec3::Z),
            vertex(c, depth, &Vec3::Z),
            vertex(b, depth, &Vec3::Z),
        ]);

        // Interior edges are shared by two triangles. Only the boundary needs walls.
        for (start, end) in [(a, b), (b, c), (c, a)] {
            let key = (start.min(end), start.max(end));
            if edges.remove(&key).is_none() {
                edges.insert(key, (start, end));
            }
        }
    }

    for (start, end) in edges.into_values() {
        let direction = geometry.vertices[end as usize] - geometry.vertices[start as usize];
        let length = direction.x.hypot(direction.y);
        if length == 0.0 {
            continue;
        }
        let normal = Vec3::new(-direction.y / length, direction.x / length, 0.0);
        vertices.extend_from_slice(&[
            vertex(start, 0.0, &normal),
            vertex(start, depth, &normal),
            vertex(end, depth, &normal),
            vertex(start, 0.0, &normal),
            vertex(end, depth, &normal),
            vertex(end, 0.0, &normal),
        ]);
    }

    Ok(vertices)
}

impl Outline {
    fn new() -> Self {
        Self {
            builder: Path::builder(),
            open: false,
        }
    }

    fn finish(mut self) -> Path {
        if self.open {
            self.builder.end(true);
        }
        self.builder.build()
    }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.open {
            self.builder.end(true);
        }
        self.builder.begin(point(x, y));
        self.open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(point(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quadratic_bezier_to(point(x1, y1), point(x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder
            .cubic_bezier_to(point(x1, y1), point(x2, y2), point(x, y));
    }

    fn close(&mut self) {
        if self.open {
            self.builder.end(true);
            self.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use ttf_parser::OutlineBuilder;

    use super::*;

    fn assert_closed_mesh(vertices: &[TextVertex]) {
        let mut edges = BTreeMap::new();
        let position = |vertex: &TextVertex| {
            [vertex[0], vertex[1], vertex[2]]
                .map(|value| if value == 0.0 { 0 } else { value.to_bits() })
        };
        for triangle in vertices.as_chunks::<3>().0 {
            let a = Vec3::from_slice(&triangle[0][..3]);
            let b = Vec3::from_slice(&triangle[1][..3]);
            let c = Vec3::from_slice(&triangle[2][..3]);
            let normal = Vec3::from_slice(&triangle[0][3..6]);
            assert!((b - a).cross(c - a).dot(normal) > 0.0);
            assert!((normal.length() - 1.0).abs() < 1e-5);

            for (start, end) in [(0, 1), (1, 2), (2, 0)] {
                let start = position(&triangle[start]);
                let end = position(&triangle[end]);
                let edge = edges
                    .entry((start.min(end), start.max(end)))
                    .or_insert((0, 0));
                edge.0 += 1;
                edge.1 += if start < end { 1 } else { -1 };
            }
        }
        assert!(!edges.is_empty());
        assert!(edges.values().all(|edge| *edge == (2, 0)));
    }

    #[test]
    fn test_extrude_holes_and_disconnected_contours() {
        let mut outline = Outline::new();
        for contour in [
            [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
            [(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)],
            [(5.0, 0.0), (6.0, 0.0), (6.0, 1.0), (5.0, 1.0)],
        ] {
            outline.move_to(contour[0].0, contour[0].1);
            for (x, y) in &contour[1..] {
                outline.line_to(*x, *y);
            }
            outline.close();
        }

        let color = [0.2, 0.4, 0.6, 1.0];
        let vertices = extrude_path(
            &outline.finish(),
            0.01,
            0.5,
            0.75,
            &Vec3::new(2.0, 3.0, 4.0),
            color,
        )
        .unwrap();
        assert_closed_mesh(&vertices);
        assert!(vertices.iter().all(|vertex| vertex[8..] == color));
        assert!(
            vertices
                .iter()
                .all(|vertex| vertex[2] == 4.0 || vertex[2] == 4.75)
        );

        let mut front_area = 0.0;
        for triangle in vertices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|triangle| triangle[0][5] == -1.0)
        {
            let a = Vec3::from_slice(&triangle[0][..3]);
            let b = Vec3::from_slice(&triangle[1][..3]);
            let c = Vec3::from_slice(&triangle[2][..3]);
            front_area += (b - a).cross(c - a).length() * 0.5;
            let hole_center = Vec3::new(3.0, 4.0, 4.0);
            assert!(
                (b - a).cross(hole_center - a).z > 0.0
                    || (c - b).cross(hole_center - b).z > 0.0
                    || (a - c).cross(hole_center - c).z > 0.0
            );
        }
        assert!((front_area - 3.25).abs() < 1e-5);
    }

    #[test]
    fn test_extrude_overlapping_contours() {
        let mut outline = Outline::new();
        for contour in [
            [(0.0, 0.0), (3.0, 0.0), (3.0, 2.0), (0.0, 2.0)],
            [(2.0, 1.0), (5.0, 1.0), (5.0, 3.0), (2.0, 3.0)],
        ] {
            outline.move_to(contour[0].0, contour[0].1);
            for (x, y) in &contour[1..] {
                outline.line_to(*x, *y);
            }
            outline.close();
        }
        let vertices =
            extrude_path(&outline.finish(), 0.01, 1.0, 0.5, &Vec3::ZERO, [1.0; 4]).unwrap();
        assert_closed_mesh(&vertices);

        let front_area: f32 = vertices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|triangle| triangle[0][5] == -1.0)
            .map(|triangle| {
                let a = Vec3::from_slice(&triangle[0][..3]);
                let b = Vec3::from_slice(&triangle[1][..3]);
                let c = Vec3::from_slice(&triangle[2][..3]);
                (b - a).cross(c - a).length() * 0.5
            })
            .sum();
        assert!((front_area - 11.0).abs() < 1e-5);
    }

    #[test]
    fn test_extrude_quadratic_and_cubic_curves() {
        let mut outline = Outline::new();
        outline.move_to(0.0, 0.0);
        outline.quad_to(2.0, -2.0, 4.0, 0.0);
        outline.curve_to(6.0, 2.0, 2.0, 6.0, 0.0, 4.0);
        outline.close();

        let vertices =
            extrude_path(&outline.finish(), 0.01, 1.0, 0.5, &Vec3::ZERO, [1.0; 4]).unwrap();
        assert_closed_mesh(&vertices);
        assert!(vertices.len() > 36);
    }

    #[test]
    fn test_extrude_font_glyphs() {
        let face = ttf_parser::Face::parse(
            include_bytes!("../../assets/liberation_sans/LiberationSans-Regular.ttf"),
            0,
        )
        .unwrap();
        let scale = 1.0 / face.units_per_em() as f32;
        for character in ['O', 'B', 'i', 'é', 'Ж'] {
            let glyph = face.glyph_index(character).unwrap();
            let vertices = extrude(&face, glyph.0, scale, 0.2, &Vec3::ZERO, [1.0; 4]).unwrap();
            assert_closed_mesh(&vertices);
        }
        let space = face.glyph_index(' ').unwrap();
        assert!(
            extrude(&face, space.0, scale, 0.2, &Vec3::ZERO, [1.0; 4])
                .unwrap()
                .is_empty()
        );
    }
}
