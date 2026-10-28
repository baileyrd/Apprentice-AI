//! Renders experiment results as CSV and a self-contained SVG. Pure strings.

use std::fmt::Write;

use crate::experiment::{Results, Snapshot, Verdict};
use crate::world::Cell;

/// One learner's results, as named on the chart.
pub struct Series<'a> {
    pub name: &'a str,
    pub results: &'a Results,
}

/// `episode,<name>...` rows, one column per series.
pub fn csv(series: &[Series]) -> String {
    let mut out = String::from("episode");
    for s in series {
        let _ = write!(out, ",{}", s.name);
    }
    out.push('\n');
    let episodes = series
        .iter()
        .map(|s| s.results.surprise.len())
        .max()
        .unwrap_or(0);
    for e in 0..episodes {
        let _ = write!(out, "{}", e + 1);
        for s in series {
            match s.results.surprise.get(e) {
                Some(v) => {
                    let _ = write!(out, ",{v:.6}");
                }
                None => out.push(','),
            }
        }
        out.push('\n');
    }
    out
}

// Reference palette (light): categorical slots 1-3, ink and surfaces.
const SERIES: [&str; 3] = ["#2a78d6", "#eb6834", "#1baf7a"];
const SURFACE: &str = "#fcfcfb";
const INK: &str = "#0b0b0b";
const INK_2: &str = "#52514e";
const MUTED: &str = "#8a8984";
const GRID: &str = "#e4e3df";
const WRONG: &str = "#e34948";

const WIDTH: f64 = 960.0;
const PAD: f64 = 24.0;
const CELL: f64 = 13.0;

fn cell_fill(cell: Cell) -> &'static str {
    match cell {
        Cell::Empty => "#ffffff",
        Cell::Wall => "#52514e",
        Cell::Food => "#1baf7a",
        Cell::Hazard => "#eda100",
    }
}

/// The surprise curve(s), the fallback guide lines and believed-map panels.
pub fn svg(series: &[Series], verdict: Option<Verdict>, deadline: usize) -> String {
    let chart_h = 300.0;
    let maps_h = series.len() as f64 * map_row_height(series) + 40.0;
    let height = 70.0 + chart_h + maps_h;
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {height}" width="{WIDTH}" height="{height}" font-family="system-ui, -apple-system, Segoe UI, sans-serif">"#
    );
    let _ = write!(s, r#"<rect width="100%" height="100%" fill="{SURFACE}"/>"#);
    let _ = write!(
        s,
        r#"<text x="{PAD}" y="32" font-size="18" font-weight="600" fill="{INK}">Surprise per episode</text><text x="{PAD}" y="52" font-size="13" fill="{INK_2}">Mean squared prediction error per square, averaged over each 100-step episode. Fixed map, seed 1.</text>"#
    );
    chart(&mut s, series, verdict, deadline, 70.0, chart_h);
    maps(&mut s, series, 70.0 + chart_h + 20.0);
    s.push_str("</svg>\n");
    s
}

fn chart(
    s: &mut String,
    series: &[Series],
    verdict: Option<Verdict>,
    deadline: usize,
    top: f64,
    h: f64,
) {
    let episodes = series
        .iter()
        .map(|s| s.results.surprise.len())
        .max()
        .unwrap_or(1)
        .max(2);
    let lines: Vec<Line> = series
        .iter()
        .map(|ser| Line {
            name: ser.name,
            points: ser
                .results
                .surprise
                .iter()
                .enumerate()
                .map(|(e, &v)| ((e + 1) as f64, v))
                .collect(),
        })
        .collect();
    let mut plot = Plot {
        top,
        height: h,
        x_range: (1.0, episodes as f64),
        x_ticks: x_ticks(episodes).into_iter().map(|e| e as f64).collect(),
        x_label: "episode",
        y_min: 0.0,
        y_max: nice_ceiling(peak(&lines).max(1e-6)),
        lines: &lines,
        h_guides: Vec::new(),
        v_guides: Vec::new(),
    };
    if let Some(v) = verdict {
        plot.h_guides = vec![
            (v.opening, "first-10 average"),
            (v.opening * 0.5, "50% target"),
        ];
    }
    if deadline <= episodes {
        plot.v_guides.push((deadline as f64, "deadline"));
    }
    line_plot(s, &plot);
}

/// One named line on a plot, as `(x, y)` points.
pub struct Line<'a> {
    pub name: &'a str,
    pub points: Vec<(f64, f64)>,
}

/// Everything a line plot needs to draw itself.
struct Plot<'a> {
    top: f64,
    height: f64,
    x_range: (f64, f64),
    x_ticks: Vec<f64>,
    x_label: &'a str,
    y_min: f64,
    y_max: f64,
    lines: &'a [Line<'a>],
    h_guides: Vec<(f64, &'a str)>,
    v_guides: Vec<(f64, &'a str)>,
}

fn peak(lines: &[Line]) -> f64 {
    lines
        .iter()
        .flat_map(|l| l.points.iter().map(|p| p.1))
        .fold(0.0_f64, f64::max)
}

/// Recessive grid, 2px lines, legend with swatches, hover titles per point.
fn line_plot(s: &mut String, p: &Plot) {
    let (left, right) = (PAD + 52.0, WIDTH - PAD - 160.0);
    let (y0, y1) = (p.top + p.height - 30.0, p.top + 10.0);
    let (xa, xb) = p.x_range;
    let x = |v: f64| left + (v - xa) / (xb - xa).max(f64::EPSILON) * (right - left);
    let y = |v: f64| y0 - (v - p.y_min) / (p.y_max - p.y_min) * (y0 - y1);

    for i in 0..=4 {
        let v = p.y_min + (p.y_max - p.y_min) * i as f64 / 4.0;
        let _ = write!(
            s,
            r#"<line x1="{left}" x2="{right}" y1="{yy:.1}" y2="{yy:.1}" stroke="{GRID}" stroke-width="1"/><text x="{tx}" y="{ty:.1}" font-size="11" fill="{INK_2}" text-anchor="end">{label}</text>"#,
            label = tick_label(v),
            yy = y(v),
            tx = left - 8.0,
            ty = y(v) + 4.0,
        );
    }
    for &t in &p.x_ticks {
        let _ = write!(
            s,
            r#"<text x="{tx:.1}" y="{ty}" font-size="11" fill="{INK_2}" text-anchor="middle">{t}</text>"#,
            tx = x(t),
            ty = y0 + 16.0,
        );
    }
    let _ = write!(
        s,
        r#"<text x="{cx:.1}" y="{ty}" font-size="11" fill="{INK_2}" text-anchor="middle">{}</text>"#,
        p.x_label,
        cx = (left + right) / 2.0,
        ty = y0 + 30.0,
    );
    for &(v, label) in &p.h_guides {
        guide(s, left, right, y(v), label);
    }
    for &(v, label) in &p.v_guides {
        let dx = x(v);
        let _ = write!(
            s,
            r#"<line x1="{dx:.1}" x2="{dx:.1}" y1="{y1}" y2="{y0}" stroke="{MUTED}" stroke-dasharray="4 4"/><text x="{tx:.1}" y="{ty}" font-size="11" fill="{INK_2}" text-anchor="end">{label}</text>"#,
            tx = dx - 4.0,
            ty = y0 - 6.0,
        );
    }
    for (i, line) in p.lines.iter().enumerate() {
        let color = SERIES[i % SERIES.len()];
        let points: Vec<String> = line
            .points
            .iter()
            .map(|&(a, b)| format!("{:.1},{:.1}", x(a), y(b)))
            .collect();
        let _ = write!(
            s,
            r#"<polyline fill="none" stroke="{color}" stroke-width="2" stroke-linejoin="round" points="{}"/>"#,
            points.join(" ")
        );
        for &(a, b) in &line.points {
            let _ = write!(
                s,
                r#"<circle cx="{:.1}" cy="{:.1}" r="5" fill="transparent"><title>{}: {} {a}, {b:.5}</title></circle>"#,
                x(a),
                y(b),
                line.name,
                p.x_label,
            );
        }
        let _ = write!(
            s,
            r#"<rect x="{lx}" y="{ly}" width="12" height="3" rx="1.5" fill="{color}"/><text x="{tx}" y="{ty}" font-size="12" fill="{INK}">{}</text>"#,
            line.name,
            lx = right + 16.0,
            ly = y1 + 6.0 + 20.0 * i as f64,
            tx = right + 34.0,
            ty = y1 + 11.0 + 20.0 * i as f64,
        );
    }
}

/// Phase 2 figure: median trailing surprise and map accuracy around the wall
/// move, then each seed's adaptation time per mind.
pub fn adaptation_svg(
    surprise: &[Line],
    accuracy: &[Line],
    times: &[(&str, Vec<Option<usize>>)],
    x_range: (f64, f64),
    horizon: usize,
) -> String {
    let panel_h = 260.0;
    let strip_h = 60.0 + 36.0 * times.len() as f64;
    let height = 70.0 + 2.0 * (panel_h + 30.0) + strip_h;
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {height}" width="{WIDTH}" height="{height}" font-family="system-ui, -apple-system, Segoe UI, sans-serif"><rect width="100%" height="100%" fill="{SURFACE}"/>"#
    );
    let _ = write!(
        s,
        r#"<text x="{PAD}" y="32" font-size="18" font-weight="600" fill="{INK}">Adapting after half the walls move</text><text x="{PAD}" y="52" font-size="13" fill="{INK_2}">Median over seeds. Step 0 is the first step after the move.</text>"#
    );
    let ticks = step_ticks(x_range);
    let surprise_top = nice_ceiling(peak(surprise).max(1e-6));
    let panels = [
        (
            surprise,
            "Surprise (mean of last 100 steps)",
            70.0,
            (0.0, surprise_top),
        ),
        (
            accuracy,
            "Share of the believed map that is right (walls, hazards, open floor)",
            70.0 + panel_h + 30.0,
            (0.8, 1.0),
        ),
    ];
    for (lines, title, top, (y_min, y_max)) in panels {
        let _ = write!(
            s,
            r#"<text x="{PAD}" y="{ty}" font-size="13" font-weight="600" fill="{INK}">{title}</text>"#,
            ty = top + 14.0,
        );
        line_plot(
            &mut s,
            &Plot {
                top: top + 20.0,
                height: panel_h - 20.0,
                x_range,
                x_ticks: ticks.clone(),
                x_label: "steps after the move",
                y_min,
                y_max,
                lines,
                h_guides: Vec::new(),
                v_guides: vec![(0.0, "walls move")],
            },
        );
    }
    strip(&mut s, times, horizon, 70.0 + 2.0 * (panel_h + 30.0));
    s.push_str("</svg>\n");
    s
}

/// One row per mind, one dot per seed at its adaptation time.
fn strip(s: &mut String, times: &[(&str, Vec<Option<usize>>)], horizon: usize, top: f64) {
    let (left, right) = (PAD + 140.0, WIDTH - PAD - 160.0);
    let x = |v: f64| left + v / horizon as f64 * (right - left);
    let _ = write!(
        s,
        r#"<text x="{PAD}" y="{ty}" font-size="13" font-weight="600" fill="{INK}">Steps to adapt, one dot per seed (hollow: not within {horizon})</text>"#,
        ty = top + 14.0,
    );
    let bottom = top + 36.0 + 36.0 * times.len() as f64;
    for t in step_ticks((0.0, horizon as f64)) {
        let _ = write!(
            s,
            r#"<line x1="{tx:.1}" x2="{tx:.1}" y1="{y1}" y2="{y2}" stroke="{GRID}"/><text x="{tx:.1}" y="{ly}" font-size="11" fill="{INK_2}" text-anchor="middle">{t}</text>"#,
            tx = x(t),
            y1 = top + 26.0,
            y2 = bottom - 8.0,
            ly = bottom + 6.0,
        );
    }
    for (i, (name, steps)) in times.iter().enumerate() {
        let color = SERIES[i % SERIES.len()];
        let cy = top + 44.0 + 36.0 * i as f64;
        let _ = write!(
            s,
            r#"<text x="{tx}" y="{ty}" font-size="12" fill="{INK}" text-anchor="end">{name}</text>"#,
            tx = left - 12.0,
            ty = cy + 4.0,
        );
        let never = steps.iter().filter(|s| s.is_none()).count();
        if never > 0 {
            let _ = write!(
                s,
                r#"<text x="{tx:.1}" y="{ty}" font-size="11" fill="{INK_2}">never ×{never}</text>"#,
                tx = x(horizon as f64) + 10.0,
                ty = cy + 4.0,
            );
        }
        for (seed, step) in steps.iter().enumerate() {
            let (cx, fill, label) = match step {
                Some(v) => (x(*v as f64), color, v.to_string()),
                None => (x(horizon as f64), SURFACE, "never".to_string()),
            };
            let _ = write!(
                s,
                r#"<circle cx="{cx:.1}" cy="{cy}" r="5" fill="{fill}" stroke="{color}" stroke-width="2"><title>{name}, seed {}: {label}</title></circle>"#,
                seed + 1,
            );
        }
    }
}

/// Up to about eight round ticks spanning `range`, including zero when inside it.
fn step_ticks((a, b): (f64, f64)) -> Vec<f64> {
    let step = nice_ceiling((b - a) / 8.0);
    let first = (a / step).ceil() as i64;
    let last = (b / step).floor() as i64;
    (first..=last).map(|k| k as f64 * step).collect()
}

fn guide(s: &mut String, left: f64, right: f64, yy: f64, label: &str) {
    let _ = write!(
        s,
        r#"<line x1="{left}" x2="{right}" y1="{yy:.1}" y2="{yy:.1}" stroke="{MUTED}" stroke-dasharray="2 4"/><text x="{tx}" y="{ty:.1}" font-size="11" fill="{INK_2}">{label}</text>"#,
        tx = right + 16.0,
        ty = yy + 4.0,
    );
}

fn map_row_height(series: &[Series]) -> f64 {
    let rows = series
        .iter()
        .flat_map(|s| s.results.snapshots.first())
        .map(|snap| snap.real.height())
        .max()
        .unwrap_or(0);
    rows as f64 * CELL + 50.0
}

fn maps(s: &mut String, series: &[Series], top: f64) {
    let row_h = map_row_height(series);
    for (i, ser) in series.iter().enumerate() {
        let y = top + i as f64 * row_h;
        let Some(last) = ser.results.snapshots.last() else {
            continue;
        };
        let panel_w = last.real.width() as f64 * CELL + 20.0;
        let label = format!("Real grid · episode {}", last.episode);
        panel(s, PAD, y, &label, &real_cells(last));
        for (j, snap) in ser.results.snapshots.iter().enumerate() {
            let label = format!("{} · episode {}", ser.name, snap.episode);
            panel(
                s,
                PAD + (j as f64 + 1.0) * panel_w,
                y,
                &label,
                &believed_cells(snap),
            );
        }
    }
    let ly = top + series.len() as f64 * row_h;
    let entries = [
        (cell_fill(Cell::Empty), "empty"),
        (cell_fill(Cell::Wall), "wall"),
        (cell_fill(Cell::Food), "food"),
        (cell_fill(Cell::Hazard), "hazard"),
        ("#e4e3df", "never seen"),
    ];
    let mut lx = PAD;
    for (fill, name) in entries {
        let _ = write!(
            s,
            r#"<rect x="{lx}" y="{ly}" width="11" height="11" fill="{fill}" stroke="{GRID}"/><text x="{tx}" y="{ty}" font-size="11" fill="{INK_2}">{name}</text>"#,
            tx = lx + 16.0,
            ty = ly + 10.0,
        );
        lx += 90.0;
    }
    let _ = write!(
        s,
        r#"<rect x="{lx}" y="{ly}" width="11" height="11" fill="none" stroke="{WRONG}" stroke-width="2"/><text x="{tx}" y="{ty}" font-size="11" fill="{INK_2}">believed wrong</text>"#,
        tx = lx + 16.0,
        ty = ly + 10.0,
    );
}

/// `(fill, wrong)` per square, row-major, with the grid width.
struct Panel {
    width: i32,
    cells: Vec<(&'static str, bool)>,
}

fn real_cells(snap: &Snapshot) -> Panel {
    Panel {
        width: snap.real.width(),
        cells: snap
            .real
            .cells()
            .iter()
            .map(|&c| (cell_fill(c), false))
            .collect(),
    }
}

fn believed_cells(snap: &Snapshot) -> Panel {
    let cells = snap
        .believed
        .iter()
        .zip(snap.real.cells())
        .map(|(b, &r)| match b {
            None => ("#e4e3df", false),
            Some(c) => (cell_fill(*c), *c != r),
        })
        .collect();
    Panel {
        width: snap.real.width(),
        cells,
    }
}

fn panel(s: &mut String, x: f64, y: f64, label: &str, p: &Panel) {
    let _ = write!(
        s,
        r#"<text x="{x}" y="{ty}" font-size="11" fill="{INK_2}">{label}</text>"#,
        ty = y + 12.0
    );
    for (i, (fill, wrong)) in p.cells.iter().enumerate() {
        let (cx, cy) = (i as i32 % p.width, i as i32 / p.width);
        let (px, py) = (x + cx as f64 * CELL, y + 20.0 + cy as f64 * CELL);
        let (stroke, sw) = if *wrong { (WRONG, 2.0) } else { (GRID, 1.0) };
        let _ = write!(
            s,
            r#"<rect x="{px}" y="{py}" width="{w}" height="{w}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#,
            w = CELL - 1.0,
        );
    }
}

/// `v` with up to four decimals, trailing zeros trimmed.
fn tick_label(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".into() } else { s.into() }
}

/// Smallest of 1, 2, 2.5, 5 × 10ⁿ at or above `v`.
fn nice_ceiling(v: f64) -> f64 {
    let base = 10f64.powf(v.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|m| m * base)
        .find(|&c| c >= v)
        .unwrap_or(10.0 * base)
}

fn x_ticks(episodes: usize) -> Vec<usize> {
    let step = [10, 20, 25, 50, 100, 200, 500]
        .into_iter()
        .find(|&s| episodes / s <= 8)
        .unwrap_or(1000);
    std::iter::once(1)
        .chain((step..=episodes).step_by(step))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ceiling_rounds_up() {
        assert_eq!(nice_ceiling(0.037), 0.05);
        assert_eq!(nice_ceiling(0.2), 0.2);
        assert_eq!(nice_ceiling(0.21), 0.25);
    }

    #[test]
    fn tick_labels_are_trimmed() {
        assert_eq!(tick_label(0.0125), "0.0125");
        assert_eq!(tick_label(0.05), "0.05");
        assert_eq!(tick_label(0.0), "0");
    }

    #[test]
    fn step_ticks_are_round_and_include_zero() {
        assert_eq!(
            step_ticks((-500.0, 3000.0)),
            vec![-500.0, 0.0, 500.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0]
        );
    }

    #[test]
    fn x_ticks_start_at_one() {
        assert_eq!(x_ticks(200), vec![1, 25, 50, 75, 100, 125, 150, 175, 200]);
    }

    #[test]
    fn csv_pads_shorter_series() {
        let a = Results {
            surprise: vec![0.5, 0.25],
            snapshots: vec![],
        };
        let b = Results {
            surprise: vec![1.0],
            snapshots: vec![],
        };
        let out = csv(&[
            Series {
                name: "a",
                results: &a,
            },
            Series {
                name: "b",
                results: &b,
            },
        ]);
        assert_eq!(out, "episode,a,b\n1,0.500000,1.000000\n2,0.250000,\n");
    }
}
