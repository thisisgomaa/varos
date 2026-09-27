//! Tiny vector symbols; no font-dependent icon glyphs.
use super::t;
use egui::{Color32, Painter, Pos2, Shape, Stroke};

#[derive(Clone, Copy)]
pub enum Icon {
    Home,
    New,
    Open,
    Remove,
    Document,
    More,
}
impl Icon {
    pub(super) fn paint(self, painter: &Painter, center: Pos2, color: Color32) {
        let path = |points: &[[f32; 2]]| {
            painter.add(Shape::line(
                points.iter().map(|[x, y]| center + egui::vec2(*x, *y) * t::KIT_ICON).collect(),
                Stroke::new(t::KIT_STROKE, color),
            ));
        };
        match self {
            Self::Home => {
                path(&[[-0.45, 0.0], [0.0, -0.4], [0.45, 0.0]]);
                path(&[
                    [-0.3, -0.1],
                    [-0.3, 0.4],
                    [-0.1, 0.4],
                    [-0.1, 0.1],
                    [0.1, 0.1],
                    [0.1, 0.4],
                    [0.3, 0.4],
                    [0.3, -0.1],
                ]);
            }
            Self::New => {
                path(&[[-0.4, 0.0], [0.4, 0.0]]);
                path(&[[0.0, -0.4], [0.0, 0.4]]);
            }
            Self::Open => path(&[
                [-0.4, 0.35],
                [-0.4, -0.35],
                [-0.1, -0.35],
                [0.05, -0.2],
                [0.4, -0.2],
                [0.4, 0.35],
                [-0.4, 0.35],
            ]),
            Self::Document => {
                path(&[[-0.3, -0.45], [0.1, -0.45], [0.35, -0.2], [0.35, 0.45], [-0.3, 0.45], [-0.3, -0.45]]);
                path(&[[0.1, -0.45], [0.1, -0.2], [0.35, -0.2]]);
            }
            Self::More => {
                for x in [-0.3, 0.0, 0.3] {
                    painter.circle_filled(center + egui::vec2(x * t::KIT_ICON, 0.0), t::KIT_STROKE, color);
                }
            }
            Self::Remove => {
                path(&[[-0.3, -0.3], [0.3, 0.3]]);
                path(&[[-0.3, 0.3], [0.3, -0.3]]);
            }
        }
    }
}
