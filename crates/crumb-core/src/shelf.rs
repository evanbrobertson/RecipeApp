//! The bookshelf's layout: which books stand and which lie down, how they gather into runs and
//! stacks, and where everything sits on each shelf. Seeded, so a shelf looks the same on every
//! load and in every app. Units are CSS px (dp in the apps).
//!
//! A title that fits up a spine stands; one that doesn't lies down, spine out, so it reads left
//! to right. Standing books gather into runs (the end one sometimes leaning on its neighbour);
//! lying books gather into stacks, longest at the foot, each a little off true.
//!
//! Ported from `web/src/lib/shelf.ts`; the parity fixtures in `tests/fixtures/app.json` hold the
//! web's own answers.

use serde::Serialize;

use crate::books::{ShelfBook, seeded};

/// DM Serif Display advance widths, thousandths of an em, for ' ' to '~' (from the font file).
#[rustfmt::skip]
const ADVANCE: [u16; 95] = [
    218, 333, 425, 557, 532, 871, 721, 223, 368, 368, 433, 541, 300, 332, 300, 342, 502, 350, 532,
    532, 538, 532, 530, 489, 502, 532, 300, 300, 541, 541, 541, 506, 877, 650, 598, 603, 648, 555,
    538, 656, 691, 301, 427, 654, 540, 791, 653, 670, 565, 670, 628, 521, 627, 649, 632, 921, 614,
    590, 560, 342, 336, 342, 541, 532, 400, 518, 581, 494, 574, 507, 343, 531, 580, 280, 281, 582,
    286, 871, 578, 545, 578, 565, 441, 436, 361, 567, 530, 781, 568, 536, 465, 338, 267, 338, 541,
];
/// Anything outside ASCII: a wide lowercase letter, so an estimate errs towards lying down.
const OTHER_ADVANCE: u16 = 580;

/// Px a title takes in DM Serif Display at `size` px (`titleWidth`).
pub fn title_width(text: &str, size: f64) -> f64 {
    let units: f64 = text
        .chars()
        .map(|c| match c as u32 {
            c @ 32..=126 => f64::from(ADVANCE[(c - 32) as usize]),
            _ => f64::from(OTHER_ADVANCE),
        })
        .sum();
    units * size / 1000.0
}

/// The shelf's fixed sizes, px (`SHELF`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfMetrics {
    /// Inside height of one shelf: the tallest book and its top edge, with a finger's room.
    pub clearance: f64,
    /// The plank: its top face, then its front edge.
    pub plank_top: f64,
    pub plank_front: f64,
    /// Space under a plank for its brackets, before the next shelf's books.
    pub under: f64,
    /// The top edge of a book (its pages, or its cover when it lies down), seen from above.
    pub top_face: f64,
    /// Space at the shelf's two ends.
    pub inset: f64,
    /// Rows are this far apart, plank included (`ROW_HEIGHT`).
    pub row_height: f64,
}

pub const METRICS: ShelfMetrics = ShelfMetrics {
    clearance: CLEARANCE,
    plank_top: 8.0,
    plank_front: 14.0,
    under: 26.0,
    top_face: 5.0,
    inset: INSET,
    row_height: CLEARANCE + 8.0 + 14.0 + 26.0,
};

const CLEARANCE: f64 = 206.0;
const INSET: f64 = 18.0;
const STAND_MIN_H: f64 = 150.0;
const STAND_MAX_H: f64 = 196.0;
const STAND_MIN_W: f64 = 32.0;
const STAND_MAX_W: f64 = 52.0;
const LIE_MIN_L: f64 = 150.0;
const LIE_MAX_L: f64 = 248.0;
const LIE_MIN_T: f64 = 30.0;
const LIE_MAX_T: f64 = 46.0;
/// Highest a stack goes, so a hand still fits over it.
const STACK_MAX: f64 = 184.0;
const POT: (f64, f64) = (46.0, 68.0);
const CROCK: (f64, f64) = (52.0, 96.0);
/// The dashed "new book" outline.
const ADD: (f64, f64) = (46.0, 160.0);

const STANDING_FONT: f64 = 15.0;
const LYING_FONT: f64 = 15.0;
const LYING_FONT_TWO: f64 = 14.0;

/// How a spine is dressed: plain cloth, gilt rules at each end, a paper label holding the
/// title, or contrasting cloth at head and foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SpineStyle {
    Plain,
    Rules,
    Label,
    Ends,
}

impl SpineStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Rules => "rules",
            Self::Label => "label",
            Self::Ends => "ends",
        }
    }

    /// Spine length that holds no title, both ends together.
    fn end_room(self) -> f64 {
        match self {
            Self::Plain => 34.0,
            Self::Rules => 52.0,
            Self::Label => 46.0,
            Self::Ends => 60.0,
        }
    }
}

pub fn spine_style(book: &ShelfBook) -> SpineStyle {
    let r = seeded(book.id, 3);
    if r < 0.3 {
        SpineStyle::Plain
    } else if r < 0.6 {
        SpineStyle::Rules
    } else if r < 0.8 {
        SpineStyle::Label
    } else {
        SpineStyle::Ends
    }
}

/// One book's spine, before it's placed. Box sizes are as drawn: `w` across, `h` up.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spine {
    /// The book's id.
    pub id: i64,
    pub standing: bool,
    pub w: f64,
    pub h: f64,
    pub style: SpineStyle,
    /// The title, as it breaks: one line, or two on a lying book.
    pub lines: Vec<String>,
    /// Px.
    pub font: f64,
}

/// JavaScript's `Math.round`: halves go up, also for negatives.
fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

/// Whether a title fits up a standing spine, and the spine it gets either way (`spineFor`).
pub fn spine_for(book: &ShelfBook) -> Spine {
    let style = spine_style(book);
    let room = style.end_room();
    let trimmed = book.name.trim_matches(js_space);
    let name = if trimmed.is_empty() {
        "Untitled"
    } else {
        trimmed
    };
    let upright = title_width(name, STANDING_FONT) + room;
    let count = book.recipe_count as f64;
    if upright <= STAND_MAX_H {
        let w = js_round(STAND_MAX_W.min(STAND_MIN_W + count * 0.8));
        let h =
            js_round(upright.max(STAND_MIN_H + seeded(book.id, 5) * (STAND_MAX_H - STAND_MIN_H)));
        return Spine {
            id: book.id,
            standing: true,
            w,
            h: h.min(STAND_MAX_H),
            style,
            lines: vec![name.to_string()],
            font: STANDING_FONT,
        };
    }
    // Too long to stand without squeezing: lay it down, on two lines if one won't do
    let mut t = js_round(LIE_MAX_T.min(LIE_MIN_T + count * 0.6));
    let mut lines = vec![name.to_string()];
    let mut font = LYING_FONT;
    let mut need = title_width(name, LYING_FONT) + room;
    if need > LIE_MAX_L {
        lines = split_title(name, LYING_FONT_TWO);
        font = LYING_FONT_TWO;
        need = lines
            .iter()
            .map(|l| title_width(l, font))
            .fold(f64::NEG_INFINITY, f64::max)
            + room;
        if lines.len() > 1 {
            t = t.max(44.0);
        }
    }
    let len = js_round(need.max(LIE_MIN_L + seeded(book.id, 5) * 40.0));
    Spine {
        id: book.id,
        standing: false,
        w: len.min(LIE_MAX_L),
        h: t,
        style,
        lines,
        font,
    }
}

/// JavaScript's whitespace, which differs from Rust's by U+FEFF (in) and U+0085 (out).
fn js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

/// Two lines with the longer one as short as it can be, broken between words (`splitTitle`).
pub fn split_title(name: &str, size: f64) -> Vec<String> {
    let words: Vec<&str> = name.split(js_space).filter(|w| !w.is_empty()).collect();
    if words.len() < 2 {
        return vec![name.to_string()];
    }
    let mut best = vec![name.to_string()];
    let mut best_width = f64::INFINITY;
    for i in 1..words.len() {
        let a = words[..i].join(" ");
        let b = words[i..].join(" ");
        let width = title_width(&a, size).max(title_width(&b, size));
        if width < best_width {
            best_width = width;
            best = vec![a, b];
        }
    }
    best
}

/// What a placed thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Book,
    /// A pot of basil, on the plank or on a stack.
    Pot,
    /// A stoneware crock of spoons.
    Crock,
    /// The dashed outline of a new book.
    Add,
}

/// The point a book turns about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pivot {
    /// Its bottom-left corner (a book leaning left rests on it).
    Left,
    /// Its bottom-right corner (a book leaning right).
    Right,
    /// Its middle (a book in a stack).
    Center,
}

/// A placed book's spine.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedSpine {
    /// Index into the books passed to [`layout_shelf`].
    pub index: usize,
    #[serde(flatten)]
    pub spine: Spine,
    /// Degrees, clockwise, about `pivot`.
    pub tilt: f64,
    pub pivot: Pivot,
    /// Any standing book, or the top book of a stack: its top edge shows.
    pub top: bool,
}

/// Where one thing sits on its shelf.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub kind: ItemKind,
    /// Left of the unturned box, from the shelf's left edge, and its foot, up from the plank.
    pub x: f64,
    pub y: f64,
    /// The unturned box: a standing book's thickness and height, a lying book's length and
    /// thickness.
    pub w: f64,
    pub h: f64,
    /// Draw order within its shelf.
    pub z: u32,
    /// Books only.
    pub book: Option<PlacedSpine>,
    /// The stack a lying book is in, or a pot sits on (unique within its shelf).
    pub stack: Option<u32>,
    /// The id of the book a pot sits on.
    pub on: Option<i64>,
}

/// One shelf, its things listed left to right and a stack's books top to bottom, so reading
/// (and tab) order follows what you see.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfRow {
    pub items: Vec<ShelfItem>,
    /// The row's width, at least the shelf's.
    pub width: f64,
}

#[derive(Debug, Clone)]
struct Indexed {
    index: usize,
    spine: Spine,
}

#[derive(Debug, Clone)]
enum Group {
    Run(Vec<Indexed>),
    Stack(Vec<Indexed>),
}

impl Group {
    fn spines(&self) -> &[Indexed] {
        match self {
            Group::Run(s) | Group::Stack(s) => s,
        }
    }
}

/// Gathers spines into runs and stacks, keeping the books' order but for a lying book, which
/// joins the stack nearest behind it (at most a few books back) rather than lie alone.
fn group_spines(spines: Vec<Indexed>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    // The open stack, as an index into `groups`
    let mut stack: Option<usize> = None;
    let mut since = 0;
    for s in spines {
        if s.spine.standing {
            let first = groups
                .last()
                .and_then(|g| g.spines().first())
                .map_or(0, |f| f.spine.id);
            let cap = 4 + (seeded(first, 19) * 5.0).floor() as usize;
            match groups.last_mut() {
                Some(Group::Run(run)) if run.len() < cap => run.push(s),
                _ => groups.push(Group::Run(vec![s])),
            }
            since += 1;
            continue;
        }
        let open = stack.and_then(|i| match &mut groups[i] {
            Group::Stack(list) => Some(list),
            Group::Run(_) => None,
        });
        match open {
            Some(list)
                if since <= 3
                    && list.iter().map(|x| x.spine.h).sum::<f64>() + s.spine.h <= STACK_MAX =>
            {
                list.push(s);
            }
            _ => {
                groups.push(Group::Stack(vec![s]));
                stack = Some(groups.len() - 1);
                since = 0;
            }
        }
    }
    groups
}

#[derive(Debug, Clone, Copy)]
struct Lean {
    /// Degrees.
    tilt: f64,
    /// Space between the neighbour and the leaning book's foot.
    gap: f64,
    /// Width the leaning book takes, gap included.
    span: f64,
}

/// A book resting on its neighbour at `deg`: its top corner against the neighbour's side, or
/// against the neighbour's top corner when it's the taller of the two.
fn lean_on(book: &Spine, neighbour: &Spine, deg: f64) -> Lean {
    let a = deg * std::f64::consts::PI / 180.0;
    let gap = (book.h * a.sin()).min(neighbour.h * a.tan());
    Lean {
        tilt: deg,
        gap,
        span: gap + book.w * a.cos(),
    }
}

#[derive(Debug, Clone)]
struct RunPlan {
    spines: Vec<Indexed>,
    /// The last book leans left on the one before it.
    end: Option<Lean>,
    /// The first book leans right on the one after it.
    start: Option<Lean>,
    width: f64,
}

/// How a run sits: books shoulder to shoulder, with an end book that may lean.
fn plan_run(spines: &[Indexed]) -> RunPlan {
    let n = spines.len();
    let mut end = None;
    let mut start = None;
    if n >= 3 {
        let last = &spines[n - 1].spine;
        if seeded(last.id, 13) < 0.5 {
            end = Some(lean_on(
                last,
                &spines[n - 2].spine,
                7.0 + seeded(last.id, 17) * 8.0,
            ));
        }
    }
    if n >= 4 && end.is_none() {
        let first = &spines[0].spine;
        if seeded(first.id, 23) < 0.4 {
            start = Some(lean_on(
                first,
                &spines[1].spine,
                6.0 + seeded(first.id, 29) * 7.0,
            ));
        }
    }
    let mut width = 0.0;
    for (i, s) in spines.iter().enumerate() {
        width += match (end, start) {
            (Some(e), _) if i == n - 1 => e.span,
            (_, Some(st)) if i == 0 => st.span,
            _ => s.spine.w + if i > 0 { 1.0 } else { 0.0 },
        };
    }
    RunPlan {
        spines: spines.to_vec(),
        end,
        start,
        width,
    }
}

fn place_run(plan: &RunPlan, x0: f64, z0: u32) -> Vec<ShelfItem> {
    let mut out = Vec::new();
    let mut x = x0;
    let n = plan.spines.len();
    for (i, s) in plan.spines.iter().enumerate() {
        let (left, tilt, pivot) = match (plan.start, plan.end) {
            (Some(st), _) if i == 0 => {
                // Pivots on its right foot, its top resting on the next book's side
                let right = x + st.span - st.gap;
                x += st.span + 1.0;
                (right - s.spine.w, st.tilt, Pivot::Right)
            }
            (_, Some(e)) if i == n - 1 => {
                let left = x + e.gap;
                x += e.span;
                (left, -e.tilt, Pivot::Left)
            }
            _ => {
                let left = x;
                x += s.spine.w + 1.0;
                (left, 0.0, Pivot::Left)
            }
        };
        out.push(book_item(
            s,
            left,
            0.0,
            z0 + i as u32,
            tilt,
            pivot,
            true,
            None,
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn book_item(
    s: &Indexed,
    x: f64,
    y: f64,
    z: u32,
    tilt: f64,
    pivot: Pivot,
    top: bool,
    stack: Option<u32>,
) -> ShelfItem {
    ShelfItem {
        kind: ItemKind::Book,
        x,
        y,
        w: s.spine.w,
        h: s.spine.h,
        z,
        book: Some(PlacedSpine {
            index: s.index,
            spine: s.spine.clone(),
            tilt,
            pivot,
            top,
        }),
        stack,
        on: None,
    }
}

#[derive(Debug, Clone)]
struct StackPlan {
    /// Bottom to top.
    spines: Vec<Indexed>,
    offsets: Vec<f64>,
    tilts: Vec<f64>,
    width: f64,
    height: f64,
}

/// Longest at the foot, mostly; each book a few px off the one below and a hair off level.
fn plan_stack(spines: &[Indexed]) -> StackPlan {
    let mut sorted = spines.to_vec();
    sorted.sort_by(|a, b| b.spine.w.total_cmp(&a.spine.w));
    // A near tie swaps now and then, so it isn't too tidy
    for i in 0..sorted.len().saturating_sub(1) {
        let (a, b) = (&sorted[i].spine, &sorted[i + 1].spine);
        if a.w - b.w < 18.0 && seeded(a.id.wrapping_add(b.id), 31) < 0.5 {
            sorted.swap(i, i + 1);
        }
    }
    let widest = sorted
        .iter()
        .map(|s| s.spine.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let offsets: Vec<f64> = sorted
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let centred = (widest - s.spine.w) / 2.0;
            if i == 0 {
                centred
            } else {
                centred + js_round((seeded(s.spine.id, 37) * 2.0 - 1.0) * 9.0)
            }
        })
        .collect();
    let tilts = sorted
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                0.0
            } else {
                js_round((seeded(s.spine.id, 41) * 2.0 - 1.0) * 12.0) / 10.0
            }
        })
        .collect();
    let left = offsets.iter().copied().fold(0.0, f64::min);
    let right = sorted
        .iter()
        .zip(&offsets)
        .map(|(s, o)| o + s.spine.w)
        .fold(f64::NEG_INFINITY, f64::max);
    StackPlan {
        height: sorted.iter().map(|s| s.spine.h).sum(),
        spines: sorted,
        offsets: offsets.iter().map(|o| o - left).collect(),
        tilts,
        width: right - left,
    }
}

fn place_stack(plan: &StackPlan, x0: f64, z0: u32) -> Vec<ShelfItem> {
    let mut y = 0.0;
    let n = plan.spines.len();
    let mut placed: Vec<ShelfItem> = plan
        .spines
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let item = book_item(
                s,
                x0 + plan.offsets[i],
                y,
                z0 + i as u32,
                plan.tilts[i],
                Pivot::Center,
                i == n - 1,
                Some(z0),
            );
            y += s.spine.h;
            item
        })
        .collect();
    // Listed top to bottom, the way the stack reads (and tabs)
    placed.reverse();
    placed
}

#[derive(Debug, Clone)]
enum Piece {
    Run(RunPlan),
    Stack(StackPlan),
    Add,
}

impl Piece {
    fn width(&self) -> f64 {
        match self {
            Piece::Run(p) => p.width,
            Piece::Stack(p) => p.width,
            Piece::Add => ADD.0,
        }
    }

    /// Space before a piece: books bunch, props get a little room.
    fn gap_before(&self, i: usize) -> f64 {
        let id = match self {
            Piece::Run(p) => p.spines[0].spine.id,
            Piece::Stack(p) => p.spines[0].spine.id,
            Piece::Add => i as i64,
        };
        js_round(14.0 + seeded(id, 43) * 20.0)
    }
}

fn row_content(pieces: &[Piece]) -> f64 {
    pieces
        .iter()
        .enumerate()
        .map(|(i, p)| p.width() + if i > 0 { p.gap_before(i) } else { 0.0 })
        .sum()
}

/// How the shelf is laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShelfOptions {
    /// The shelf's inside width.
    pub width: f64,
    /// One row as long as it needs, for a shelf that scrolls sideways.
    pub single: bool,
    /// End with a dashed outline for a new book.
    pub addable: bool,
}

/// Lays the books out on as many shelves as the width needs (`layoutShelf`).
pub fn layout_shelf(books: &[ShelfBook], opts: ShelfOptions) -> Vec<ShelfRow> {
    let groups = group_spines(
        books
            .iter()
            .enumerate()
            .map(|(index, b)| Indexed {
                index,
                spine: spine_for(b),
            })
            .collect(),
    );
    if opts.single {
        let rows = fill(&groups, f64::INFINITY, opts);
        return vec![place_row(&rows[0], 0, opts, f64::INFINITY)];
    }
    let inner = (opts.width - INSET * 2.0).max(120.0);
    // Fill shelves greedily, then again to an even share each so the last one isn't left bare
    let mut rows = fill(&groups, inner, opts);
    if rows.len() > 1 {
        let total: f64 = rows.iter().map(|r| row_content(r)).sum();
        let widest = rows
            .iter()
            .flatten()
            .map(Piece::width)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut share = widest.max(total / rows.len() as f64);
        while share < inner {
            let even = fill(&groups, share, opts);
            if even.len() == rows.len() {
                rows = even;
                break;
            }
            share += 16.0;
        }
    }
    rows.iter()
        .enumerate()
        .map(|(ri, pieces)| place_row(pieces, ri, opts, inner))
        .collect()
}

/// Fills shelves up to `inner` wide, splitting a run of standing books that doesn't fit.
fn fill(groups: &[Group], inner: f64, opts: ShelfOptions) -> Vec<Vec<Piece>> {
    let mut rows: Vec<Vec<Piece>> = vec![Vec::new()];
    let fits = |rows: &[Vec<Piece>], p: &Piece| {
        let row = rows.last().expect("a row");
        let gap = if row.is_empty() {
            0.0
        } else {
            p.gap_before(row.len())
        };
        row_content(row) + gap + p.width() <= inner
    };
    let push = |rows: &mut Vec<Vec<Piece>>, p: Piece| {
        if !fits(rows, &p) && !rows.last().expect("a row").is_empty() {
            rows.push(Vec::new());
        }
        rows.last_mut().expect("a row").push(p);
    };
    for g in groups {
        let rest = match g {
            Group::Stack(spines) => {
                push(&mut rows, Piece::Stack(plan_stack(spines)));
                continue;
            }
            Group::Run(spines) => spines,
        };
        let mut rest: &[Indexed] = rest;
        while !rest.is_empty() {
            let whole = Piece::Run(plan_run(rest));
            if fits(&rows, &whole) {
                push(&mut rows, whole);
                break;
            }
            // As many as fit here; the rest start the next shelf
            let mut k = rest.len() - 1;
            while k > 0 && !fits(&rows, &Piece::Run(plan_run(&rest[..k]))) {
                k -= 1;
            }
            if k == 0 {
                if !rows.last().expect("a row").is_empty() {
                    rows.push(Vec::new());
                    continue;
                }
                k = 1;
            }
            push(&mut rows, Piece::Run(plan_run(&rest[..k])));
            rest = &rest[k..];
            rows.push(Vec::new());
        }
    }
    if rows.len() > 1 && rows.last().expect("a row").is_empty() {
        rows.pop();
    }
    if opts.addable {
        push(&mut rows, Piece::Add);
    }
    rows
}

fn thing(kind: ItemKind, x: f64, y: f64, (w, h): (f64, f64), z: u32) -> ShelfItem {
    ShelfItem {
        kind,
        x,
        y,
        w,
        h,
        z,
        book: None,
        stack: None,
        on: None,
    }
}

fn place_row(pieces: &[Piece], ri: usize, opts: ShelfOptions, inner: f64) -> ShelfRow {
    let gaps: Vec<f64> = pieces
        .iter()
        .enumerate()
        .map(|(i, p)| if i > 0 { p.gap_before(i) } else { 0.0 })
        .collect();
    let content = row_content(pieces);
    let width = if inner.is_finite() { inner } else { content };
    let mut spare = (width - content).max(0.0);

    // A herb pot on the first shelf: on a stack with headroom, else on the plank if there's room
    let mut pot_on: Option<usize> = None;
    let mut pot_alone = false;
    if ri == 0 {
        let mut best: Option<(usize, f64)> = None;
        for (i, p) in pieces.iter().enumerate() {
            let Piece::Stack(plan) = p else { continue };
            if plan.height + POT.1 > CLEARANCE - 6.0 {
                continue;
            }
            if best.is_none_or(|(_, h)| plan.height < h) {
                best = Some((i, plan.height));
            }
        }
        if let Some((i, _)) = best {
            pot_on = Some(i);
        } else if spare >= POT.0 + 24.0 || opts.single {
            pot_alone = true;
        }
    }
    if pot_alone {
        spare = (spare - (POT.0 + 24.0)).max(0.0);
    }
    // A crock of spoons where a lower shelf has room to spare
    let crock = ri > 0 && spare >= CROCK.0 + 60.0 && seeded(ri as i64, 47) < 0.6;
    if crock {
        spare = (spare - (CROCK.0 + 24.0)).max(0.0);
    }

    // Loosen the gaps a little, then centre what's left
    let slack = spare.min(pieces.len().saturating_sub(1) as f64 * 14.0);
    let each = if pieces.len() > 1 {
        slack / (pieces.len() - 1) as f64
    } else {
        0.0
    };
    let mut x = INSET + (spare - slack) / 2.0;

    let mut items: Vec<ShelfItem> = Vec::new();
    let mut z: u32 = 0;
    fn pot_x(items: &mut Vec<ShelfItem>, x: &mut f64, z: &mut u32) {
        items.push(thing(ItemKind::Pot, *x, 0.0, POT, *z));
        *z += 1;
        *x += POT.0 + 24.0;
    }
    // The pot goes at the left end on some rows and at the right on others
    if pot_alone && seeded(pieces.len() as i64, 53) < 0.5 {
        pot_x(&mut items, &mut x, &mut z);
    }
    for (i, p) in pieces.iter().enumerate() {
        if i > 0 {
            x += gaps[i] + each;
        }
        match p {
            Piece::Run(plan) => {
                let placed = place_run(plan, x, z);
                z += placed.len() as u32;
                items.extend(placed);
            }
            Piece::Stack(plan) => {
                let placed = place_stack(plan, x, z);
                z += placed.len() as u32;
                let top = placed[0].clone();
                items.extend(placed);
                if pot_on == Some(i) {
                    let at = top.x + top.w / 2.0 - POT.0 / 2.0
                        + (seeded((ri + i) as i64, 59) * 2.0 - 1.0) * 20.0;
                    let mut pot = thing(ItemKind::Pot, at, plan.height, POT, z);
                    pot.stack = top.stack;
                    pot.on = top.book.as_ref().map(|b| b.spine.id);
                    items.push(pot);
                    z += 1;
                }
            }
            Piece::Add => {
                items.push(thing(ItemKind::Add, x, 0.0, ADD, z));
                z += 1;
            }
        }
        x += p.width();
    }
    if pot_alone && !items.iter().any(|it| it.kind == ItemKind::Pot) {
        x += 24.0;
        pot_x(&mut items, &mut x, &mut z);
    }
    if crock {
        x += 24.0;
        items.push(thing(ItemKind::Crock, x, 0.0, CROCK, z));
        x += CROCK.0;
    }
    let end = if opts.single {
        x + INSET
    } else {
        width + INSET * 2.0
    };
    ShelfRow { items, width: end }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(id: i64, name: &str, count: i64) -> ShelfBook {
        ShelfBook {
            id,
            name: name.into(),
            color: None,
            recipe_count: count,
        }
    }

    #[test]
    fn short_titles_stand_and_long_ones_lie_down() {
        assert!(spine_for(&book(1, "Soups", 3)).standing);
        let long = spine_for(&book(
            2,
            "Grandma's Sunday roasts and other family favourites",
            3,
        ));
        assert!(!long.standing);
        assert_eq!(long.lines.len(), 2);
        assert_eq!(spine_for(&book(3, "  ", 0)).lines, vec!["Untitled"]);
    }

    #[test]
    fn every_book_is_placed_once() {
        let books: Vec<_> = (1..=20)
            .map(|i| {
                book(
                    i,
                    if i % 3 == 0 {
                        "A rather long cookbook title here"
                    } else {
                        "Pasta"
                    },
                    i,
                )
            })
            .collect();
        for width in [320.0, 700.0, 1200.0] {
            let rows = layout_shelf(
                &books,
                ShelfOptions {
                    width,
                    single: false,
                    addable: true,
                },
            );
            let mut seen: Vec<usize> = rows
                .iter()
                .flat_map(|r| &r.items)
                .filter_map(|it| it.book.as_ref().map(|b| b.index))
                .collect();
            seen.sort_unstable();
            assert_eq!(seen, (0..20).collect::<Vec<_>>());
            let adds = rows
                .iter()
                .flat_map(|r| &r.items)
                .filter(|it| it.kind == ItemKind::Add);
            assert_eq!(adds.count(), 1);
        }
    }

    #[test]
    fn an_empty_shelf_still_has_room_for_a_new_book() {
        let rows = layout_shelf(
            &[],
            ShelfOptions {
                width: 600.0,
                single: false,
                addable: true,
            },
        );
        assert_eq!(rows.len(), 1);
        assert!(rows[0].items.iter().any(|it| it.kind == ItemKind::Add));
    }

    /// Each thing standing on the plank as an x-range: a stack counts as one, a pot on a stack
    /// not at all.
    fn footprints(row: &ShelfRow) -> Vec<(f64, f64)> {
        let mut out: Vec<(f64, f64)> = Vec::new();
        let mut stacks: std::collections::HashMap<u32, (f64, f64)> = Default::default();
        for it in &row.items {
            match (it.stack, it.on) {
                (_, Some(_)) => {}
                (Some(s), None) => {
                    let e = stacks.entry(s).or_insert((it.x, it.x + it.w));
                    e.0 = e.0.min(it.x);
                    e.1 = e.1.max(it.x + it.w);
                }
                (None, None) => out.push((it.x, it.x + it.w)),
            }
        }
        out.extend(stacks.into_values());
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    #[test]
    fn nothing_runs_off_a_shelf_or_overlaps() {
        for n in [0, 1, 2, 3, 5, 8, 13, 30, 60] {
            for (names, count) in [
                (&["Pasta"][..], 60),
                (&["Pasta", "Soups", "Bread"][..], 3),
                (
                    &[
                        "Pasta",
                        "Grandma's Sunday roasts and other family favourites",
                    ][..],
                    12,
                ),
            ] {
                let books: Vec<_> = (0..n)
                    .map(|i| book(100 + i as i64 * 7, names[i % names.len()], count))
                    .collect();
                for width in [280.0, 320.0, 375.0, 414.0, 700.0, 1100.0, 1600.0] {
                    let opts = ShelfOptions {
                        width,
                        single: false,
                        addable: true,
                    };
                    for row in layout_shelf(&books, opts) {
                        let fp = footprints(&row);
                        let alone = fp.len() == 1;
                        for it in &row.items {
                            assert!(it.x >= 0.0, "{n} books at {width}: x {}", it.x);
                            assert!(
                                alone || it.x + it.w <= row.width + 0.5,
                                "{n} books at {width}: item ends at {} on a {} row",
                                it.x + it.w,
                                row.width
                            );
                        }
                        for pair in fp.windows(2) {
                            assert!(
                                pair[1].0 >= pair[0].1 - 0.5,
                                "{n} books at {width}: {pair:?} overlap"
                            );
                        }
                    }
                }
                let single = ShelfOptions {
                    width: 0.0,
                    single: true,
                    addable: false,
                };
                for row in layout_shelf(&books, single) {
                    for pair in footprints(&row).windows(2) {
                        assert!(
                            pair[1].0 >= pair[0].1 - 0.5,
                            "{n} books in one row: {pair:?} overlap"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn titles_split_on_javascripts_whitespace() {
        assert_eq!(split_title("\u{feff}Sunday  roast\u{85}pie", 14.0).len(), 2);
        assert_eq!(spine_for(&book(1, "\u{feff}", 0)).lines, vec!["Untitled"]);
    }
}
