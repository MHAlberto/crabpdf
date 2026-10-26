//! Cache and join adjacent glyph rectangles while the text selection is unchanged.
use crate::{
    application::selection::TextSelection,
    document::{Bounds, PageContent},
};
#[derive(Default)]
pub(super) struct SelectionGeometry {
    selection: Option<TextSelection>,
    boxes: Vec<Bounds>,
}
impl SelectionGeometry {
    pub fn get(&mut self, selection: TextSelection, content: &PageContent) -> &[Bounds] {
        if self.selection == Some(selection) {
            return &self.boxes;
        }
        self.selection = Some(selection);
        self.boxes.clear();
        for bounds in content
            .glyphs
            .iter()
            .filter(|glyph| selection.contains(glyph.index))
            .filter_map(|glyph| glyph.bounds)
        {
            if let Some(last) = self.boxes.last_mut() {
                let height = (last.bottom - last.top).min(bounds.bottom - bounds.top);
                let same_line = (last.top - bounds.top).abs() <= height * 0.1
                    && (last.bottom - bounds.bottom).abs() <= height * 0.1;
                let adjacent = bounds.left <= last.right + height * 0.2
                    && bounds.right >= last.left - height * 0.2;
                if same_line && adjacent {
                    last.left = last.left.min(bounds.left);
                    last.right = last.right.max(bounds.right);
                    last.top = last.top.min(bounds.top);
                    last.bottom = last.bottom.max(bounds.bottom);
                    continue;
                }
            }
            self.boxes.push(bounds);
        }
        &self.boxes
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crabpdf_core::document::Glyph;
    #[test]
    fn selected_lines_merge_but_columns_and_different_lines_remain_separate() {
        let glyph = |index, x, y| Glyph {
            index,
            text: "a".into(),
            bounds: Some(Bounds {
                left: x,
                right: x + 0.01,
                top: y,
                bottom: y + 0.02,
            }),
        };
        let content = PageContent {
            glyphs: vec![
                glyph(0, 0.10, 0.1),
                glyph(1, 0.11, 0.1),
                glyph(2, 0.12, 0.1),
                glyph(3, 0.5, 0.1),
                glyph(4, 0.1, 0.2),
            ],
            ..Default::default()
        };
        let mut geometry = SelectionGeometry::default();
        let selection = TextSelection {
            page: 0,
            start: 4,
            end: 0,
        };
        let boxes = geometry.get(selection, &content);
        assert_eq!(boxes.len(), 3);
        assert_eq!(boxes[0].left, 0.1);
        assert_eq!(boxes[0].right, 0.13);
        let ptr = boxes.as_ptr();
        assert_eq!(geometry.get(selection, &content).as_ptr(), ptr);
        assert_eq!(
            geometry
                .get(
                    TextSelection {
                        start: 1,
                        end: 1,
                        ..selection
                    },
                    &content
                )
                .len(),
            1
        );
    }
}
