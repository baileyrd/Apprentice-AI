//! Revisit: does a planner-side pull toward long-unseen squares make new
//! barren worlds easier, at no cost elsewhere? Each seed is lived in the
//! shared life of `apprentice::life`. Marks are in docs/revisit/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::apprentice::Drives;
use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, Setting, lives_under};
use apprentice::memory::Recall;

const USAGE: &str = "\
usage: revisit [--judge RATE] [--out DIR]

Without --judge: tuning on seeds 101-110, today's default against revisit
1/64, 1/128, 1/256 and 1/512, and the rate the tuning rule picks. With
--judge RATE: the judging seeds 1-100 and 141-240, today's default (A)
against revisit RATE (R), and each mark's verdict. Writes DIR/revisit.csv
(default DIR: runs/revisit).";

type Error = Box<dyn std::error::Error>;

/// Rates tried in tuning.
const CANDIDATES: [f64; 4] = [1.0 / 64.0, 1.0 / 128.0, 1.0 / 256.0, 1.0 / 512.0];
/// Most bush test sleeps a tuned rate may cost, against the default.
const BUSH_GUARD: f64 = 1.1;

/// Parsed flags.
struct Args {
    judge: Option<f64>,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut judge = None;
    let mut out = PathBuf::from("runs/revisit");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--judge" => {
                let rate: f64 = value.parse().map_err(|_| format!("bad --judge: {value}"))?;
                if !(rate > 0.0 && rate <= 1.0) {
                    return Err("--judge needs a rate in (0, 1]".into());
                }
                judge = Some(rate);
            }
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    Ok(Args { judge, out })
}

/// Per-seed measures, in CSV column order. Sleeps and meals in the barren
/// test are per 1,000 steps.
const MEASURES: [Measure; 11] = [
    ("barren_sleeps_per_1k", |l| f64::from(l.barren.sleeps) / 2.0),
    ("barren_meals_per_1k", |l| f64::from(l.barren.meals) / 2.0),
    ("first_barren_sleeps", |l| {
        f64::from(l.barren_raised[0].sleeps)
    }),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("raised_bush_sleeps", |l| f64::from(l.raised_sleeps(false))),
    ("raised_barren_sleeps", |l| f64::from(l.raised_sleeps(true))),
    ("home_wrong_100", |l| f64::from(l.home.0)),
    ("barren_hopes", |l| f64::from(l.barren.hopes)),
    ("bush_hopes", |l| f64::from(l.bush.hopes)),
    ("bush_meals", |l| f64::from(l.bush.meals)),
    ("barren_futile", |l| f64::from(l.barren.futile)),
];

/// One seed and its life under each setting.
type Row = (u64, Vec<Life>);

fn setting(revisit: f64) -> Setting {
    Setting {
        drives: Drives {
            revisit,
            ..Drives::default()
        },
        ..Setting::from(Recall::default())
    }
}

fn column(rows: &[Row], s: usize, f: fn(&Life) -> f64) -> Vec<f64> {
    rows.iter().map(|r| f(&r.1[s])).collect()
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn csv(rows: &[Row], names: &[String]) -> String {
    let heads: Vec<&str> = MEASURES.iter().map(|m| m.0).collect();
    let mut out = format!("seed,setting,{}\n", heads.join(","));
    for (seed, lives) in rows {
        for (name, l) in names.iter().zip(lives) {
            let values: Vec<String> = MEASURES.iter().map(|m| m.1(l).to_string()).collect();
            out.push_str(&format!("{seed},{name},{}\n", values.join(",")));
        }
    }
    out
}

/// Tuning: each candidate's means, and the rate the rule picks.
fn tune(rows: &[Row]) -> Option<f64> {
    let base_bush = mean(&column(rows, 0, MEASURES[3].1));
    println!(
        "default: barren sleeps/1k {:.2}, bush sleeps {:.2}",
        mean(&column(rows, 0, MEASURES[0].1)),
        base_bush
    );
    let mut pick: Option<(f64, f64)> = None;
    for (i, &rate) in CANDIDATES.iter().enumerate() {
        let barren = mean(&column(rows, i + 1, MEASURES[0].1));
        let bush = mean(&column(rows, i + 1, MEASURES[3].1));
        let meals = mean(&column(rows, i + 1, MEASURES[1].1));
        println!(
            "revisit 1/{:.0}: barren sleeps/1k {barren:.2}, barren meals/1k {meals:.2}, bush sleeps {bush:.2}",
            1.0 / rate
        );
        let allowed = bush <= BUSH_GUARD * base_bush;
        if allowed && pick.is_none_or(|(_, best)| barren < best) {
            pick = Some((rate, barren));
        }
    }
    pick.map(|(rate, _)| rate)
}

fn show(i: &Interval, raw: &Interval) -> String {
    format!(
        "{:+.0}% [{:+.0}%, {:+.0}%], raw {:+.2} [{:+.2}, {:+.2}]",
        100.0 * i.mid,
        100.0 * i.lo,
        100.0 * i.hi,
        raw.mid,
        raw.lo,
        raw.hi
    )
}

/// Paired change and raw difference of `f` from `base` to `test` columns.
fn pair(base: &[f64], test: &[f64]) -> Result<(Interval, Interval), Error> {
    let diffs: Vec<f64> = test.iter().zip(base).map(|(t, b)| t - b).collect();
    Ok((Interval::change(base, test)?, Interval::mean(&diffs)?))
}

fn judge(rows: &[Row]) -> Result<(), Error> {
    let (a, r) = (0, 1);
    let both = |f: fn(&Life) -> f64| pair(&column(rows, a, f), &column(rows, r, f));
    let (c, raw) = both(MEASURES[0].1)?;
    println!(
        "1. barren test sleeps/1k, R vs A {}: {}",
        show(&c, &raw),
        c.at_most(-0.1)
    );
    let (c, raw) = both(MEASURES[1].1)?;
    println!(
        "2. barren test meals/1k, R vs A {}: {}",
        show(&c, &raw),
        c.at_least(0.1)
    );
    let (c, raw) = pair(
        &column(rows, r, MEASURES[2].1),
        &column(rows, r, MEASURES[0].1),
    )?;
    println!(
        "3. R barren test vs first barren world, sleeps/1k {}: {}",
        show(&c, &raw),
        c.at_most(0.0)
    );
    println!("4. R vs A, each at most +10%:");
    for (name, f) in &MEASURES[3..7] {
        let (c, raw) = both(*f)?;
        println!("   {name} {}: {}", show(&c, &raw), c.at_most(0.1));
    }
    println!("Reported only, R vs A:");
    for (name, f) in &MEASURES[7..] {
        let (c, raw) = both(*f)?;
        println!("   {name} {}", show(&c, &raw));
    }
    println!("Means (A, R):");
    for (name, f) in MEASURES {
        println!(
            "   {name}: {:.2}, {:.2}",
            mean(&column(rows, a, f)),
            mean(&column(rows, r, f))
        );
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let (seeds, rates): (Vec<u64>, Vec<f64>) = match args.judge {
        Some(rate) => ((1..=100).chain(141..=240).collect(), vec![rate]),
        None => ((101..=110).collect(), CANDIDATES.to_vec()),
    };
    let settings: Vec<Setting> = std::iter::once(0.0)
        .chain(rates.iter().copied())
        .map(setting)
        .collect();
    let rows = lives_under(&seeds, &settings)?;
    let names: Vec<String> = std::iter::once("A".to_string())
        .chain(rates.iter().map(|r| format!("R{:.0}", 1.0 / r)))
        .collect();
    match args.judge {
        Some(_) => judge(&rows)?,
        None => match tune(&rows) {
            Some(rate) => println!("picked: revisit 1/{:.0}", 1.0 / rate),
            None => println!("picked: none (every rate costs over 10% bush sleeps)"),
        },
    }
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("revisit.csv"), csv(&rows, &names))?;
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(&args) {
        eprintln!("revisit: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
