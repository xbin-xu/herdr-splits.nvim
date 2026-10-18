//! Directional candidate search over tiled rectangles.
//!
//! This is the tmux `select-pane` target rule shared by the Neovim side
//! (`lua/herdr-splits/geometry.lua`) and the Herdr side (`geometry.candidates`
//! here, formerly `_pane_candidates` in `herdr_common.py`): among the
//! rectangles adjacent in a direction, take the nearest layer (smallest
//! positive gap) that overlaps perpendicularly; `wrap` flips the search to the
//! opposite layout boundary.
//!
//! Results preserve the caller's order, which is the server's pane order. That
//! matters: when no history exists the first candidate wins, and the original
//! implementations both relied on "server order, deterministic".

/// A pane (or window) rectangle in terminal cells. Inclusive edges:
/// `x .. x + w - 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl Rect {
    pub fn new(x: i64, y: i64, w: i64, h: i64) -> Self {
        Rect { x, y, w, h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Left => "left",
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::Right => "right",
        }
    }

    pub fn parse(text: &str) -> Option<Direction> {
        match text {
            "left" => Some(Direction::Left),
            "down" => Some(Direction::Down),
            "up" => Some(Direction::Up),
            "right" => Some(Direction::Right),
            _ => None,
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
        }
    }

    fn horizontal(self) -> bool {
        matches!(self, Direction::Left | Direction::Right)
    }

    fn forward(self) -> bool {
        matches!(self, Direction::Right | Direction::Down)
    }
}

/// One searchable rectangle: a stable id plus its geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub id: String,
    pub rect: Rect,
}

impl Cell {
    pub fn new(id: impl Into<String>, rect: Rect) -> Self {
        Cell {
            id: id.into(),
            rect,
        }
    }
}

/// Project a rect onto the movement axis. Returns
/// `(start, end, perpendicular_start, perpendicular_end)` with inclusive
/// edges, collapsing the four-direction matrix to `horizontal`/`forward`.
fn project(rect: Rect, horizontal: bool) -> (i64, i64, i64, i64) {
    if horizontal {
        (rect.x, rect.x + rect.w - 1, rect.y, rect.y + rect.h - 1)
    } else {
        (rect.y, rect.y + rect.h - 1, rect.x, rect.x + rect.w - 1)
    }
}

/// Ids of the rectangles the tmux directional search considers from `cur`.
///
/// Returns an empty vector when `cur` is not part of `cells`. `wrap` flips the
/// search across the layout boundary (the opposite edge), which is how both
/// Herdr-pane and Neovim-window navigation wrap.
pub fn candidates(cells: &[Cell], cur: &str, direction: Direction, wrap: bool) -> Vec<String> {
    let Some(current) = cells.iter().find(|cell| cell.id == cur) else {
        return Vec::new();
    };

    let horizontal = direction.horizontal();
    let forward = direction.forward();
    // The edge facing the movement (tmux's edge comparison).
    let near = |start: i64, end: i64| if forward { start } else { end };
    let (cs, ce, cps, cpe) = project(current.rect, horizontal);

    if wrap {
        // Opposite boundary: right/down wrap to the smallest start coordinate,
        // left/up wrap to the largest stop coordinate.
        let mut boundary: Option<i64> = None;
        for cell in cells {
            let (start, end, _, _) = project(cell.rect, horizontal);
            let edge = near(start, end);
            boundary = Some(match boundary {
                None => edge,
                Some(current_boundary) if forward => current_boundary.min(edge),
                Some(current_boundary) => current_boundary.max(edge),
            });
        }
        let Some(boundary) = boundary else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for cell in cells {
            if cell.id == cur {
                continue;
            }
            let (start, end, ps, pe) = project(cell.rect, horizontal);
            if near(start, end) == boundary && ps <= cpe && pe >= cps {
                out.push(cell.id.clone());
            }
        }
        return out;
    }

    // Nearest layer beyond the current window: smallest positive gap.
    let mut best_gap: Option<i64> = None;
    let mut out = Vec::new();
    for cell in cells {
        if cell.id == cur {
            continue;
        }
        let (start, end, ps, pe) = project(cell.rect, horizontal);
        let gap = if forward { start - ce } else { cs - end };
        if gap >= 1 && ps <= cpe && pe >= cps {
            match best_gap {
                None => {
                    best_gap = Some(gap);
                    out = vec![cell.id.clone()];
                }
                Some(current_gap) if gap < current_gap => {
                    best_gap = Some(gap);
                    out = vec![cell.id.clone()];
                }
                Some(current_gap) if gap == current_gap => out.push(cell.id.clone()),
                _ => {}
            }
        }
    }
    out
}

/// True when the layout still describes a real tiling.
///
/// Herdr keeps reporting the pre-zoom pane rectangles while a tab is zoomed,
/// so the directional search works unchanged. Older servers collapsed a zoomed
/// layout to the zoomed pane's full-area rectangle (every edge reported
/// reached); with fewer than two distinct rects there is nothing to search, and
/// callers fall back to Herdr's geometric focus.
pub fn layout_is_tiled(cells: &[Cell]) -> bool {
    if cells.len() <= 1 {
        return false;
    }
    let first = cells[0].rect;
    cells.iter().any(|cell| cell.rect != first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(id: &str, x: i64, y: i64, w: i64, h: i64) -> Cell {
        Cell::new(id, Rect::new(x, y, w, h))
    }

    /// Candidate ids joined with commas, so the assertions read like the
    /// fixtures they came from (`"b,c"`).
    fn got(cells: &[Cell], cur: &str, direction: Direction, wrap: bool) -> String {
        candidates(cells, cur, direction, wrap).join(",")
    }

    /// Fixtures transcribed from `tests/test_geometry.lua`, which pins the rule
    /// the Lua side and the Python side already agree on.
    #[test]
    fn picks_the_nearest_layer_preserving_order_on_ties() {
        let cells = [
            cell("a", 0, 0, 10, 20),
            cell("b", 10, 0, 10, 10),
            cell("c", 10, 10, 10, 10),
        ];
        assert_eq!(got(&cells, "a", Direction::Right, false), "b,c");
        assert_eq!(got(&cells, "c", Direction::Left, false), "a");
        assert_eq!(got(&cells, "b", Direction::Down, false), "c");
        assert!(candidates(&cells, "b", Direction::Up, false).is_empty());
    }

    #[test]
    fn nearest_layer_wins_over_farther_windows() {
        let cells = [
            cell("a", 0, 0, 10, 20),
            cell("b", 10, 0, 10, 10),
            cell("d", 30, 0, 10, 10),
        ];
        assert_eq!(got(&cells, "a", Direction::Right, false), "b");
        assert_eq!(got(&cells, "b", Direction::Right, false), "d");
    }

    #[test]
    fn requires_perpendicular_overlap() {
        let cells = [
            cell("a", 0, 0, 10, 10),
            cell("b", 10, 0, 10, 10),
            cell("c", 10, 10, 10, 10),
        ];
        assert_eq!(got(&cells, "a", Direction::Right, false), "b");
    }

    #[test]
    fn wrap_flips_to_the_opposite_layout_boundary() {
        let cells = [
            cell("a", 0, 0, 10, 20),
            cell("b", 10, 0, 10, 10),
            cell("c", 10, 10, 10, 10),
            // Left boundary, but no perpendicular overlap with b/c.
            cell("d", 0, 20, 10, 10),
        ];
        assert_eq!(got(&cells, "a", Direction::Left, true), "b,c");
        assert_eq!(got(&cells, "b", Direction::Right, true), "a");
        // c already sits on the right boundary: right-wrap looks at the left
        // boundary and takes only the row-overlapping window (a, not d).
        assert_eq!(got(&cells, "c", Direction::Right, true), "a");

        let stack = [
            cell("top", 0, 0, 10, 10),
            cell("mid", 0, 10, 10, 10),
            cell("bot", 0, 20, 10, 10),
        ];
        assert_eq!(got(&stack, "top", Direction::Up, true), "bot");
        assert_eq!(got(&stack, "bot", Direction::Down, true), "top");
    }

    #[test]
    fn without_wrap_there_are_no_candidates_at_the_boundary() {
        let cells = [cell("a", 0, 0, 10, 20), cell("b", 10, 0, 10, 10)];
        assert!(candidates(&cells, "a", Direction::Left, false).is_empty());
        assert!(candidates(&cells, "b", Direction::Right, false).is_empty());
    }

    #[test]
    fn returns_nothing_when_the_current_rect_is_absent() {
        let cells = [cell("a", 0, 0, 10, 10), cell("b", 10, 0, 10, 10)];
        assert!(candidates(&cells, "ghost", Direction::Right, false).is_empty());
        assert!(candidates(&cells, "ghost", Direction::Right, true).is_empty());
        assert!(candidates(&[], "a", Direction::Right, false).is_empty());
    }

    #[test]
    fn one_cell_split_separators_are_absorbed() {
        // A one-cell gap between neighbours is still "the nearest layer".
        let cells = [cell("a", 0, 0, 10, 10), cell("b", 11, 0, 10, 10)];
        assert_eq!(got(&cells, "a", Direction::Right, false), "b");
    }

    #[test]
    fn layout_is_tiled_needs_two_distinct_rects() {
        assert!(!layout_is_tiled(&[]));
        assert!(!layout_is_tiled(&[cell("a", 0, 0, 80, 24)]));
        assert!(!layout_is_tiled(&[
            cell("a", 0, 0, 80, 24),
            cell("b", 0, 0, 80, 24)
        ]));
        assert!(layout_is_tiled(&[
            cell("a", 0, 0, 40, 24),
            cell("b", 40, 0, 40, 24)
        ]));
    }

    #[test]
    fn direction_parsing_and_opposites() {
        assert_eq!(Direction::parse("left"), Some(Direction::Left));
        assert_eq!(Direction::parse("LEFT"), None);
        assert_eq!(Direction::parse("sideways"), None);
        assert_eq!(Direction::Left.opposite(), Direction::Right);
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::Right.as_str(), "right");
    }
}
