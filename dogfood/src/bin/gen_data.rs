// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generate the CSV fixtures used by the dogfood tasks (no gup dependency).

use chrono::{Duration, NaiveDate};
use std::fs::File;
use std::io::{BufWriter, Write};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn normal(&mut self) -> f64 {
        let (u1, u2) = (self.next().max(1e-12), self.next());
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

fn main() -> std::io::Result<()> {
    std::fs::create_dir_all("/tmp/gup-dogfood")?;
    let mut rng = Lcg(42);

    // Task 1: ~5 years of daily prices, wide format, a few blanks.
    let mut w = BufWriter::new(File::create("/tmp/gup-dogfood/prices.csv")?);
    writeln!(w, "date,AAPL,MSFT,GOOG")?;
    let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let mut p = [75.0f64, 160.0, 68.0];
    for day in 0..(5 * 365 + 1) {
        let d = start + Duration::days(day);
        let mut cells = Vec::new();
        for (i, price) in p.iter_mut().enumerate() {
            *price *= 1.0 + 0.0004 + 0.018 * rng.normal();
            let missing = (day * 7 + i as i64 * 13) % 211 == 0;
            cells.push(if missing { String::new() } else { format!("{price:.2}") });
        }
        writeln!(w, "{},{}", d.format("%Y-%m-%d"), cells.join(","))?;
    }
    w.flush()?;

    // Task 2: sales by region x quarter (long format).
    let mut w = BufWriter::new(File::create("/tmp/gup-dogfood/sales.csv")?);
    writeln!(w, "region,quarter,sales")?;
    for region in ["North", "South", "East", "West"] {
        for q in ["Q1", "Q2", "Q3", "Q4"] {
            writeln!(w, "{region},{q},{:.0}", 80.0 + 120.0 * rng.next())?;
        }
    }
    w.flush()?;

    // Task 3: 200k points, category + size, x spans 4 decades.
    let mut w = BufWriter::new(File::create("/tmp/gup-dogfood/points.csv")?);
    writeln!(w, "id,income,spend,segment,weight")?;
    let segs = ["retail", "wholesale", "online", "partner", "other"];
    for id in 0..200_000 {
        let s = (rng.next() * segs.len() as f64) as usize;
        let income = 10f64.powf(3.0 + 4.0 * rng.next());
        let spend = income.log10() * 10.0 + s as f64 * 5.0 + 4.0 * rng.normal();
        let weight = 1.0 + 9.0 * rng.next();
        writeln!(w, "{id},{income:.2},{spend:.3},{},{weight:.2}", segs[s])?;
    }
    w.flush()?;
    println!("wrote /tmp/gup-dogfood/{{prices,sales,points}}.csv");
    Ok(())
}
