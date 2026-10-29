//! Startup choices share the same rectangles for rendering and hit testing.
use crate::control_layout::Rect;
pub struct Layout {
    pub panel: Rect,
    pub title: Rect,
    pub buttons: [Rect; 2],
}
impl Layout {
    pub fn new(w: i32, h: i32) -> Option<Self> {
        if !(160..=8192).contains(&w) || !(100..=8192).contains(&h) {
            return None;
        }
        let width = (w - 32).clamp(128, 640);
        let height = (h - 16).clamp(84, 220);
        let panel = Rect {
            x: (w - width) / 2,
            y: (h - height) / 2,
            w: width,
            h: height,
        };
        let gap = (height / 16).clamp(4, 12);
        let title_h = height / 4;
        let button_h = (height - title_h - 3 * gap) / 2;
        let title = Rect {
            x: panel.x + gap,
            y: panel.y + gap,
            w: width - 2 * gap,
            h: title_h,
        };
        let first = Rect {
            x: title.x,
            y: title.y + title_h + gap,
            w: title.w,
            h: button_h,
        };
        let second = Rect {
            y: first.y + button_h + gap,
            ..first
        };
        Some(Self {
            panel,
            title,
            buttons: [first, second],
        })
    }
    pub fn hit(&self, x: f64, y: f64) -> Option<bool> {
        if self.buttons[0].contains(x, y) {
            Some(false)
        } else if self.buttons[1].contains(x, y) {
            Some(true)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_fit_small_windows_and_share_hit_geometry() {
        for (w, h) in [(160, 100), (640, 360), (1920, 1080), (3840, 2160)] {
            let l = Layout::new(w, h).unwrap();
            for (i, b) in l.buttons.iter().enumerate() {
                assert!(b.x >= 0 && b.y >= 0 && b.x + b.w <= w && b.y + b.h <= h);
                assert_eq!(
                    l.hit((b.x + b.w / 2) as f64, (b.y + b.h / 2) as f64),
                    Some(i == 1)
                );
            }
            assert_eq!(l.hit(0.0, 0.0), None);
        }
    }
}
