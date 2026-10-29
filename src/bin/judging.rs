//! Judging the judging: calibrates the paired method on spare seeds, then
//! re-judges the rules of the language (#14), trust (#16) and peers (#17)
//! phases on held-out seeds, under the old method (10-seed medians) and the
//! new one (paired typical change with a 90% interval).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use apprentice::judge::{Interval, JudgeError, Verdict};
use apprentice::rng::Rng;

const USAGE: &str = "\
usage: judging [--dir DIR]

Reads DIR/calibration/{foretell,peers}.csv (seeds 2001-2200) and
DIR/pr14/foretell.csv, DIR/pr16/foretell.csv, DIR/pr17/peers.csv (judging
seeds), as written by the foretell and peers binaries at each PR's commit.
Prints the calibration checks and every rule's verdict. Default DIR:
docs/judging.";

type Error = Box<dyn std::error::Error>;

/// Calibration seeds.
const CALIBRATION: std::ops::Range<u64> = 2001..2201;
/// Mark for the false-alarm check: "at least 15% fewer".
const ALARM: f64 = -0.15;
/// Most interval width allowed when choosing the number of seeds.
const WIDEST: f64 = 0.30;

/// Per-seed rows of one CSV: (key, seed) -> column -> value.
struct Table {
    rows: HashMap<(String, u64), HashMap<String, f64>>,
}

impl Table {
    /// Parses a CSV whose first column is a key that may itself hold commas
    /// (the minds "both, one voice") and whose second is the seed.
    fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().ok_or("empty csv")?.split(',').collect();
        let mut rows = HashMap::new();
        for line in lines.filter(|l| !l.is_empty()) {
            let mut fields: Vec<&str> = line.rsplitn(header.len(), ',').collect();
            fields.reverse();
            let seed: u64 = fields.get(1).ok_or("short row")?.parse()?;
            let vals = header[2..]
                .iter()
                .zip(&fields[2..])
                .map(|(h, v)| (h.to_string(), v.parse().unwrap_or(f64::NAN)))
                .collect();
            rows.insert((fields[0].to_string(), seed), vals);
        }
        Ok(Self { rows })
    }

    fn read(path: &Path) -> Result<Self, Error> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    /// Seeds present for `key`, in order.
    fn seeds(&self, key: &str) -> Vec<u64> {
        let mut s: Vec<u64> = self
            .rows
            .keys()
            .filter(|(k, _)| k == key)
            .map(|&(_, s)| s)
            .collect();
        s.sort_unstable();
        s
    }

    /// `col` of `key` over `seeds`; NaN where missing.
    fn col(&self, key: &str, col: &str, seeds: &[u64]) -> Vec<f64> {
        seeds
            .iter()
            .map(|&s| {
                self.rows
                    .get(&(key.to_string(), s))
                    .and_then(|r| r.get(col).copied())
                    .unwrap_or(f64::NAN)
            })
            .collect()
    }
}

/// One way of turning per-seed values into a verdict.
trait Method {
    /// Is the change from `base` to `test` at most `mark` (e.g. -0.25)?
    fn change(&self, base: &[f64], test: &[f64], mark: f64) -> Verdict;
    /// Is the share of `hits` at least `mark`?
    fn share(&self, hits: &[bool], mark: f64) -> Verdict;
    /// Is the typical level of `xs` at most `mark`?
    fn level(&self, xs: &[f64], mark: f64) -> Verdict;
}

/// The old way: medians compared once, pass or fail.
struct Medians;

/// The new way: paired typical change with a 90% interval, three verdicts.
struct Paired;

fn pass(ok: bool) -> Verdict {
    if ok { Verdict::Met } else { Verdict::Missed }
}

fn median(xs: &[f64]) -> f64 {
    Interval::median(xs).map_or(f64::NAN, |i| i.mid)
}

impl Method for Medians {
    fn change(&self, base: &[f64], test: &[f64], mark: f64) -> Verdict {
        pass(median(test) <= median(base) * (1.0 + mark))
    }
    fn share(&self, hits: &[bool], mark: f64) -> Verdict {
        pass(hits.iter().filter(|&&h| h).count() as f64 >= mark * hits.len() as f64)
    }
    fn level(&self, xs: &[f64], mark: f64) -> Verdict {
        pass(median(xs) <= mark)
    }
}

impl Method for Paired {
    fn change(&self, base: &[f64], test: &[f64], mark: f64) -> Verdict {
        Interval::change(base, test).map_or(Verdict::Unclear, |i| i.at_most(mark))
    }
    fn share(&self, hits: &[bool], mark: f64) -> Verdict {
        let n = hits.iter().filter(|&&h| h).count();
        Interval::share(n, hits.len()).map_or(Verdict::Unclear, |i| i.at_least(mark))
    }
    fn level(&self, xs: &[f64], mark: f64) -> Verdict {
        Interval::median(xs).map_or(Verdict::Unclear, |i| i.at_most(mark))
    }
}

/// Met only if every part is met; missed if any part is missed.
fn all(parts: &[Verdict]) -> Verdict {
    match () {
        _ if parts.contains(&Verdict::Missed) => Verdict::Missed,
        _ if parts.iter().all(|&v| v == Verdict::Met) => Verdict::Met,
        _ => Verdict::Unclear,
    }
}

/// Met if any part is met; missed only if every part is missed.
fn any(parts: &[Verdict]) -> Verdict {
    match () {
        _ if parts.contains(&Verdict::Met) => Verdict::Met,
        _ if parts.iter().all(|&v| v == Verdict::Missed) => Verdict::Missed,
        _ => Verdict::Unclear,
    }
}

/// A phase's data on one set of seeds.
struct View<'a> {
    table: &'a Table,
    seeds: &'a [u64],
}

impl View<'_> {
    fn col(&self, key: &str, col: &str) -> Vec<f64> {
        self.table.col(key, col, self.seeds)
    }
}

/// A rule: its name, the original verdict on seeds 1-10, and how to judge it.
struct Rule {
    name: &'static str,
    was: &'static str,
    judge: fn(&dyn Method, &Data) -> Verdict,
}

/// Every table a rule may read, on one set of seeds.
struct Data<'a> {
    /// Language and trust minds, from `foretell`.
    minds: View<'a>,
    /// The same minds on the code before (for "no regressions").
    before: Option<View<'a>>,
    /// Peers cases, from `peers`.
    peers: Option<View<'a>>,
}

const NONE: &str = "no mentor";
const TRUE: &str = "true mentor";
const RANDOM: &str = "random mentor";
const ONE_VOICE: &str = "both, one voice";
const TWO_VOICES: &str = "both, two voices";

fn change(m: &dyn Method, v: &View, base: &str, test: &str, col: &str, mark: f64) -> Verdict {
    m.change(&v.col(base, col), &v.col(test, col), mark)
}

fn peers_change(m: &dyn Method, d: &Data, base: &str, test: &str, col: &str, mark: f64) -> Verdict {
    d.peers
        .as_ref()
        .map_or(Verdict::Unclear, |v| change(m, v, base, test, col, mark))
}

const LANGUAGE: [Rule; 4] = [
    Rule {
        name: "1. Words learned in at least 90% of seeds",
        was: "missed (7/10)",
        judge: |m, d| {
            let g = d.minds.col(TRUE, "grows_back");
            let s = d.minds.col(TRUE, "stays_gone");
            let hits: Vec<bool> = g
                .iter()
                .zip(&s)
                .map(|(&g, &s)| g >= 0.8 && s <= 0.2)
                .collect();
            m.share(&hits, 0.9)
        },
    },
    Rule {
        name: "2. Futile returns at least 50% below no mentor",
        was: "missed (+7%)",
        judge: |m, d| change(m, &d.minds, NONE, TRUE, "barren_futile", -0.5),
    },
    Rule {
        name: "3. Bush sleeps no more than 10% above no mentor",
        was: "met (-41%)",
        judge: |m, d| change(m, &d.minds, NONE, TRUE, "bush_sleeps", 0.1),
    },
    Rule {
        name: "4. Random mentor's futile returns no more than 10% above",
        was: "missed (+33%)",
        judge: |m, d| change(m, &d.minds, NONE, RANDOM, "barren_futile", 0.1),
    },
];

const TRUST: [Rule; 4] = [
    Rule {
        name: "1. Honest trust 0.3 above random in at least 90% of seeds",
        was: "missed (4/10)",
        judge: |m, d| {
            let a = d.minds.col(TWO_VOICES, "trust_0");
            let b = d.minds.col(TWO_VOICES, "trust_1");
            let hits: Vec<bool> = a.iter().zip(&b).map(|(a, b)| a - b >= 0.3).collect();
            m.share(&hits, 0.9)
        },
    },
    Rule {
        name: "2. Two voices: false hopes at most 2; bush sleeps within 10% of honest alone",
        was: "missed (6.5; 9.5 vs 4.5)",
        judge: |m, d| {
            all(&[
                m.level(&d.minds.col(TWO_VOICES, "barren_hopes"), 2.0),
                change(m, &d.minds, TRUE, TWO_VOICES, "bush_sleeps", 0.1),
            ])
        },
    },
    Rule {
        name: "3. Two voices: false hopes at least 50% below one voice",
        was: "met (-61%)",
        judge: |m, d| change(m, &d.minds, ONE_VOICE, TWO_VOICES, "barren_hopes", -0.5),
    },
    Rule {
        name: "4. No language-test sleeps more than 10% above the code before",
        was: "met",
        judge: |m, d| {
            let Some(before) = &d.before else {
                return Verdict::Unclear;
            };
            let parts: Vec<Verdict> = [NONE, TRUE, RANDOM]
                .iter()
                .flat_map(|&mind| {
                    ["barren_sleeps", "bush_sleeps"]
                        .map(|col| m.change(&before.col(mind, col), &d.minds.col(mind, col), 0.1))
                })
                .collect();
            all(&parts)
        },
    },
];

const ALONE: &str = "alone";
const TOGETHER: &str = "together";
const ELDER: &str = "elder ahead";
const STRANGER: &str = "stranger ahead";

const PEERS: [Rule; 4] = [
    Rule {
        name: "1. Together: false hopes -25%, bush sleeps -15%",
        was: "missed (-22%; +35%)",
        judge: |m, d| {
            all(&[
                peers_change(m, d, ALONE, TOGETHER, "barren_hopes", -0.25),
                peers_change(m, d, ALONE, TOGETHER, "bush_sleeps", -0.15),
            ])
        },
    },
    Rule {
        name: "2. Elder: false hopes -50%, bush sleeps -25%",
        was: "missed (-11%; +18%)",
        judge: |m, d| {
            all(&[
                peers_change(m, d, ALONE, ELDER, "barren_hopes", -0.5),
                peers_change(m, d, ALONE, ELDER, "bush_sleeps", -0.25),
            ])
        },
    },
    Rule {
        name: "3. Sibling elder 30% below stranger on one measure, within 10% on the other",
        was: "missed (-11%; -23%)",
        judge: |m, d| {
            let c = |col, mark| peers_change(m, d, STRANGER, ELDER, col, mark);
            any(&[
                all(&[c("barren_hopes", -0.3), c("bush_sleeps", 0.1)]),
                all(&[c("bush_sleeps", -0.3), c("barren_hopes", 0.1)]),
            ])
        },
    },
    Rule {
        name: "4. No condition's sleeps more than 10% above alone",
        was: "missed (11.5, 13 vs 8.5)",
        judge: |m, d| {
            let parts: Vec<Verdict> = [TOGETHER, ELDER, STRANGER]
                .iter()
                .flat_map(|&case| {
                    ["barren_sleeps", "bush_sleeps"]
                        .map(|col| peers_change(m, d, ALONE, case, col, 0.1))
                })
                .collect();
            all(&parts)
        },
    },
];

/// Interval widths for bush sleeps at the first `n` calibration seeds.
fn widths(minds: &Table, peers: &Table) -> Vec<(usize, f64, f64)> {
    let seeds: Vec<u64> = CALIBRATION.collect();
    [50, 100, 200]
        .into_iter()
        .map(|n| {
            let s = &seeds[..n];
            let w = |t: &Table, a: &str, b: &str| {
                Interval::change(&t.col(a, "bush_sleeps", s), &t.col(b, "bush_sleeps", s))
                    .map_or(f64::NAN, |i| i.width())
            };
            (n, w(minds, NONE, TRUE), w(peers, ALONE, ELDER))
        })
        .collect()
}

/// Seeds in a random order.
fn shuffled(seeds: &[u64], rng: &mut Rng) -> Vec<u64> {
    let mut s = seeds.to_vec();
    for i in (1..s.len()).rev() {
        s.swap(i, rng.below(i + 1));
    }
    s
}

/// Share of do-nothing comparisons (no mentor against itself on other
/// seeds) each method calls "at least 15% fewer bush sleeps" (old, new),
/// and share whose interval leaves out "no change" (ideally about 10%).
fn false_alarms(minds: &Table, splits: usize) -> (f64, f64, f64) {
    let seeds: Vec<u64> = CALIBRATION.collect();
    let mut rng = Rng::new(7);
    let (mut old, mut new, mut off) = (0, 0, 0);
    for _ in 0..splits {
        let s = shuffled(&seeds, &mut rng);
        let half = s.len() / 2;
        let a = minds.col(NONE, "bush_sleeps", &s[..half]);
        let b = minds.col(NONE, "bush_sleeps", &s[half..]);
        old += usize::from(Medians.change(&a[..10], &b[..10], ALARM) == Verdict::Met);
        new += usize::from(Paired.change(&a, &b, ALARM) == Verdict::Met);
        off += usize::from(Paired.change(&a, &b, 0.0) != Verdict::Unclear);
    }
    let share = |k: usize| k as f64 / splits as f64;
    (share(old), share(new), share(off))
}

fn judge_all(m: &dyn Method, d: &Data, rules: &[Rule]) -> Vec<Verdict> {
    rules.iter().map(|r| (r.judge)(m, d)).collect()
}

/// Rules readable on today's code: language, trust 1-3, peers.
fn todays(m: &dyn Method, d: &Data) -> Vec<Verdict> {
    [
        judge_all(m, d, &LANGUAGE),
        judge_all(m, d, &TRUST[..3]),
        judge_all(m, d, &PEERS),
    ]
    .concat()
}

/// Met against missed between two verdict lists.
fn flips(a: &[Verdict], b: &[Verdict]) -> usize {
    a.iter()
        .zip(b)
        .filter(|(x, y)| {
            matches!(
                (x, y),
                (Verdict::Met, Verdict::Missed) | (Verdict::Missed, Verdict::Met)
            )
        })
        .count()
}

/// Reproducibility: (flips between the two halves under the new method,
/// rules checked, mean flips between disjoint 10-seed groups, old method).
fn reproducibility(minds: &Table, peers: &Table) -> (usize, usize, f64) {
    let seeds: Vec<u64> = CALIBRATION.collect();
    let data = |s: &[u64]| -> (Vec<Verdict>, Vec<Verdict>) {
        let d = Data {
            minds: View {
                table: minds,
                seeds: s,
            },
            before: None,
            peers: Some(View {
                table: peers,
                seeds: s,
            }),
        };
        (todays(&Medians, &d), todays(&Paired, &d))
    };
    let (half, rest) = seeds.split_at(seeds.len() / 2);
    let (a, b) = (data(half).1, data(rest).1);
    let groups: Vec<Vec<Verdict>> = seeds.chunks(10).map(|g| data(g).0).collect();
    let pairs = groups.len() / 2;
    let old: usize = (0..pairs)
        .map(|i| flips(&groups[2 * i], &groups[2 * i + 1]))
        .sum();
    (flips(&a, &b), a.len(), old as f64 / pairs as f64)
}

fn print_rules(title: &str, rules: &[Rule], d: &Data, first10: &Data) {
    println!("\n{title}");
    for r in rules {
        println!(
            "  {:<80} was {:<26} old on 1-10: {:<8} new: {}",
            r.name,
            r.was,
            (r.judge)(&Medians, first10).to_string(),
            (r.judge)(&Paired, d)
        );
    }
}

/// Prints a typical change with its interval.
fn show(label: &str, change: Result<Interval, JudgeError>) {
    match change {
        Ok(i) => println!(
            "  {label:<52} {:>+5.0}%  [{:+.0}%, {:+.0}%]",
            100.0 * i.mid,
            100.0 * i.lo,
            100.0 * i.hi
        ),
        Err(e) => println!("  {label:<52} {e}"),
    }
}

/// Typical change of `col` from `base` to `test` in one view.
fn show_change(label: &str, v: &View, base: &str, test: &str, col: &str) {
    show(
        label,
        Interval::change(&v.col(base, col), &v.col(test, col)),
    );
}

fn execute(dir: &Path) -> Result<(), Error> {
    let cal_minds = Table::read(&dir.join("calibration/foretell.csv"))?;
    let cal_peers = Table::read(&dir.join("calibration/peers.csv"))?;
    println!("Interval width for bush sleeps (want at most {WIDEST:.2}):");
    for (n, a, b) in widths(&cal_minds, &cal_peers) {
        println!("  {n:>3} seeds: true mentor vs none {a:.3}, elder vs alone {b:.3}");
    }
    let (old, new, off) = false_alarms(&cal_minds, 200);
    println!(
        "Do-nothing change called 'at least 15% fewer bush sleeps': old {:.0}%, new {:.1}%; new interval leaves out no change in {:.0}%",
        100.0 * old,
        100.0 * new,
        100.0 * off
    );
    let (flipped, checked, old_flips) = reproducibility(&cal_minds, &cal_peers);
    println!(
        "Met/missed flips between seeds 2001-2100 and 2101-2200: {flipped} of {checked} rules (old method between two groups of 10 seeds: {old_flips:.1} on average)"
    );

    let pr14 = Table::read(&dir.join("pr14/foretell.csv"))?;
    let pr16 = Table::read(&dir.join("pr16/foretell.csv"))?;
    let pr17 = Table::read(&dir.join("pr17/peers.csv"))?;
    let seeds = pr14.seeds(NONE);
    println!(
        "\nJudging seeds: {} ({}-{})",
        seeds.len(),
        seeds[0],
        seeds[seeds.len() - 1]
    );
    let first10 = &seeds[..10];
    let view = |t, s| View { table: t, seeds: s };
    let lang = |s| Data {
        minds: view(&pr14, s),
        before: None,
        peers: None,
    };
    let trust = |s| Data {
        minds: view(&pr16, s),
        before: Some(view(&pr14, s)),
        peers: None,
    };
    let peers = |s| Data {
        minds: view(&pr16, s),
        before: None,
        peers: Some(view(&pr17, s)),
    };
    print_rules("Language (#14)", &LANGUAGE, &lang(&seeds), &lang(first10));
    print_rules("Trust (#16)", &TRUST, &trust(&seeds), &trust(first10));
    print_rules("Peers (#17)", &PEERS, &peers(&seeds), &peers(first10));

    println!("\nTypical changes on the judging seeds, with 90% intervals:");
    let (l, t, p) = (
        view(&pr14, &seeds),
        view(&pr16, &seeds),
        view(&pr17, &seeds),
    );
    show_change(
        "#14 true mentor vs none: futile returns",
        &l,
        NONE,
        TRUE,
        "barren_futile",
    );
    show_change(
        "#14 true mentor vs none: false hopes",
        &l,
        NONE,
        TRUE,
        "barren_hopes",
    );
    show_change(
        "#14 true mentor vs none: bush sleeps",
        &l,
        NONE,
        TRUE,
        "bush_sleeps",
    );
    show_change(
        "#14 random mentor vs none: futile returns",
        &l,
        NONE,
        RANDOM,
        "barren_futile",
    );
    show_change(
        "#14 random mentor vs none: bush sleeps",
        &l,
        NONE,
        RANDOM,
        "bush_sleeps",
    );
    show_change(
        "#16 two voices vs honest alone: bush sleeps",
        &t,
        TRUE,
        TWO_VOICES,
        "bush_sleeps",
    );
    show_change(
        "#16 two voices vs one voice: false hopes",
        &t,
        ONE_VOICE,
        TWO_VOICES,
        "barren_hopes",
    );
    show_change(
        "#16 two voices vs one voice: bush sleeps",
        &t,
        ONE_VOICE,
        TWO_VOICES,
        "bush_sleeps",
    );
    for mind in [NONE, TRUE, RANDOM] {
        for col in ["barren_sleeps", "bush_sleeps"] {
            let label = format!("#16 vs #14 code, {mind}: {col}");
            show(
                &label,
                Interval::change(&pr14.col(mind, col, &seeds), &pr16.col(mind, col, &seeds)),
            );
        }
    }
    for case in [TOGETHER, ELDER, STRANGER] {
        for col in ["barren_hopes", "barren_sleeps", "bush_sleeps"] {
            show_change(&format!("#17 {case} vs alone: {col}"), &p, ALONE, case, col);
        }
    }
    for col in ["barren_hopes", "bush_sleeps"] {
        show_change(
            &format!("#17 elder vs stranger: {col}"),
            &p,
            STRANGER,
            ELDER,
            col,
        );
    }
    let pct = |xs: Vec<f64>| Interval::median(&xs).map_or(f64::NAN, |i| i.mid);
    println!(
        "  #16 two voices: median false hopes {:.1}",
        pct(t.col(TWO_VOICES, "barren_hopes"))
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.as_slice() {
        [] => PathBuf::from("docs/judging"),
        [flag, d] if flag == "--dir" => PathBuf::from(d),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(&dir) {
        eprintln!("judging: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_may_hold_commas_and_blanks_read_as_nan() {
        let t = Table::parse("mind,seed,a,b\nboth, two voices,3,1.5,\nnone,3,2,4\n").unwrap();
        let a = t.col("both, two voices", "a", &[3]);
        assert_eq!(a, vec![1.5]);
        assert!(t.col("both, two voices", "b", &[3])[0].is_nan());
        assert_eq!(t.seeds("none"), vec![3]);
    }

    #[test]
    fn combined_verdicts() {
        use Verdict::*;
        assert_eq!(all(&[Met, Unclear]), Unclear);
        assert_eq!(all(&[Met, Missed]), Missed);
        assert_eq!(any(&[Missed, Met]), Met);
        assert_eq!(any(&[Missed, Unclear]), Unclear);
    }
}
