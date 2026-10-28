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

// Reference palette (light): categorical slots 1-2, ink and surfaces.
const SERIES: [&str; 2] = ["#2a78d6", "#eb6834"];
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
    let (left, right) = (PAD + 52.0, WIDTH - PAD - 140.0);
    let (y0, y1) = (top + h - 30.0, top + 10.0);
    let episodes = series
        .iter()
        .map(|s| s.results.surprise.len())
        .max()
        .unwrap_or(1)
        .max(2);
    let peak = series
        .iter()
        .flat_map(|s| s.results.surprise.iter().copied())
        .fold(0.0_f64, f64::max);
    let y_max = nice_ceiling(peak.max(1e-6));
    let x = |e: f64| left + (e - 1.0) / (episodes as f64 - 1.0) * (right - left);
    let y = |v: f64| y0 - v / y_max * (y0 - y1);

    // Recessive grid and axes.
    for i in 0..=4 {
        let v = y_max * i as f64 / 4.0;
        let _ = write!(
            s,
            r#"<line x1="{left}" x2="{right}" y1="{yy:.1}" y2="{yy:.1}" stroke="{GRID}" stroke-width="1"/><text x="{tx}" y="{ty:.1}" font-size="11" fill="{INK_2}" text-anchor="end">{label}</text>"#,
            label = tick_label(v),
            yy = y(v),
            tx = left - 8.0,
            ty = y(v) + 4.0,
        );
    }
    for e in x_ticks(episodes) {
        let _ = write!(
            s,
            r#"<text x="{tx:.1}" y="{ty}" font-size="11" fill="{INK_2}" text-anchor="middle">{e}</text>"#,
            tx = x(e as f64),
            ty = y0 + 16.0,
        );
    }
    let _ = write!(
        s,
        r#"<text x="{cx:.1}" y="{ty}" font-size="11" fill="{INK_2}" text-anchor="middle">episode</text>"#,
        cx = (left + right) / 2.0,
        ty = y0 + 30.0,
    );

    // Fallback guides: opening average, the 50% target, the deadline.
    if let Some(v) = verdict {
        guide(s, left, right, y(v.opening), "first-10 average");
        guide(s, left, right, y(v.opening * 0.5), "50% target");
    }
    if deadline <= episodes {
        let dx = x(deadline as f64);
        let _ = write!(
            s,
            r#"<line x1="{dx:.1}" x2="{dx:.1}" y1="{y1}" y2="{y0}" stroke="{MUTED}" stroke-dasharray="4 4"/><text x="{tx:.1}" y="{ty}" font-size="11" fill="{INK_2}" text-anchor="end">deadline</text>"#,
            tx = dx - 4.0,
            ty = y1 + 10.0,
        );
    }

    // Series: 2px lines, direct label at the end, hover titles per episode.
    for (i, ser) in series.iter().enumerate() {
        let color = SERIES[i % SERIES.len()];
        let points: Vec<String> = ser
            .results
            .surprise
            .iter()
            .enumerate()
            .map(|(e, &v)| format!("{:.1},{:.1}", x(e as f64 + 1.0), y(v)))
            .collect();
        let _ = write!(
            s,
            r#"<polyline fill="none" stroke="{color}" stroke-width="2" stroke-linejoin="round" points="{}"/>"#,
            points.join(" ")
        );
        for (e, &v) in ser.results.surprise.iter().enumerate() {
            let _ = write!(
                s,
                r#"<circle cx="{:.1}" cy="{:.1}" r="5" fill="transparent"><title>{}: episode {}, surprise {v:.5}</title></circle>"#,
                x(e as f64 + 1.0),
                y(v),
                ser.name,
                e + 1
            );
        }
        let _ = write!(
            s,
            r#"<rect x="{lx}" y="{ly}" width="12" height="3" rx="1.5" fill="{color}"/><text x="{tx}" y="{ty}" font-size="12" fill="{INK}">{}</text>"#,
            ser.name,
            lx = right + 16.0,
            ly = y1 + 6.0 + 20.0 * i as f64,
            tx = right + 34.0,
            ty = y1 + 11.0 + 20.0 * i as f64,
        );
    }
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
