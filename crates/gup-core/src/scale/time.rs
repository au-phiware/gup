// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Linear, LinearParams, PositionScale, Ticks, step_value, tick_step};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::Result;
use crate::shader::{SCALE_TIME, WgslModule};

const MINUTE: f64 = 60.0;
const HOUR: f64 = 3_600.0;
const DAY: f64 = 86_400.0;
/// Mean Gregorian month and year, for choosing an interval only.
const MONTH: f64 = 30.436_875 * DAY;
const YEAR: f64 = 365.242_5 * DAY;

/// A time position scale: linear over f64 seconds since the Unix epoch
/// (UTC), with calendar ticks.
///
/// Ticks land on whole calendar units chosen from the domain's span —
/// fractions of a second, seconds, minutes, hours, days, weeks (Mondays),
/// months, quarters or years — and each is labelled by the coarsest unit
/// it begins: `2021` on a year, `Mar` on a month, `Feb 29` on a day,
/// `14:00` on an hour, `14:05` on a minute, `:30` on a second and `.250`
/// within one. Times are UTC; leap seconds do not exist in Unix time.
///
/// Reads [`ColumnFormat::F32x2Relative`] (hi/lo) columns, so positions stay
/// within a quarter pixel of the f64 mirror however deep the zoom: a chunk
/// of 2^20 one-per-second samples zoomed to one millisecond across 1000 px
/// is still exact (GUP-418).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Time {
    linear: Linear,
}

impl Time {
    /// A scale whose domain is fitted to the data (and made nice).
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a fixed domain (seconds) instead of fitting the data.
    pub fn domain(mut self, d0: f64, d1: f64) -> Self {
        self.linear = self.linear.domain(d0, d1);
        self
    }

    /// Set the pixel range.
    pub fn range(mut self, r0: Px, r1: Px) -> Self {
        self.linear = self.linear.range(r0, r1);
        self
    }

    fn resolved_domain(&self) -> (f64, f64) {
        self.linear.current_domain().unwrap_or((0.0, DAY))
    }
}

impl ShaderFn for Time {
    type In = f32;
    type Out = Px;
    /// The same layout as [`Linear`]'s: `gup::scale::time::Params`.
    type Params = LinearParams;
    const MODULE: &'static WgslModule = &SCALE_TIME;
    const ENTRY: &'static str = "map_rel";

    fn params(&self) -> LinearParams {
        self.linear.params()
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32x2Relative
    }

    /// `origin - d0`, computed in f64 (and split into hi/lo by the
    /// column format).
    fn chunk_base(&self, origin: f64) -> f64 {
        ShaderFn::chunk_base(&self.linear, origin)
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        self.linear.fit_domain(extent)
    }
}

impl CpuMirror for Time {
    fn eval(&self, x: f64) -> f64 {
        self.linear.eval(x)
    }
}

impl PositionScale for Time {
    fn current_domain(&self) -> Option<(f64, f64)> {
        self.linear.current_domain()
    }

    fn is_auto(&self) -> bool {
        self.linear.is_auto()
    }

    fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()> {
        self.linear.set_domain(d0, d1)
    }

    fn set_range(&mut self, r0: Px, r1: Px) {
        self.linear.set_range(r0, r1);
    }

    fn invert(&self, px: f64) -> f64 {
        self.linear.invert(px)
    }

    /// Extend the domain to the calendar interval ten ticks would use
    /// (whole days, months, years, …).
    fn nice(&mut self) {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let (lo, hi) = if lo == hi {
            (lo - DAY / 2.0, hi + DAY / 2.0)
        } else {
            let unit = Interval::choose(hi - lo, 10);
            (unit.floor(lo), unit.ceil(hi))
        };
        self.linear
            .replace_domain(if d0 <= d1 { (lo, hi) } else { (hi, lo) });
    }

    /// Calendar ticks: about `count` of them, on whole units of the
    /// interval the span calls for, labelled by the coarsest unit each
    /// begins.
    fn ticks(&self, count: usize) -> Ticks {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let unit = Interval::choose(hi - lo, count);
        let values = unit.ticks(lo, hi);
        let labels = values.iter().map(|&t| label(t, &unit)).collect();
        Ticks { values, labels }
    }
}

/// A calendar tick interval.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Interval {
    /// A 1–2–5 step under a second.
    Fraction(f64),
    /// A fixed number of seconds that divides a day (or a day, or two),
    /// aligned to midnight UTC.
    Seconds(f64),
    /// Weeks, starting on Mondays.
    Week,
    /// `n` months (1, 3 or 6), aligned to January.
    Months(u32),
    /// A 1–2–5 step of whole years.
    Years(f64),
}

impl Interval {
    /// The interval giving closest to `count` ticks over `span` seconds.
    fn choose(span: f64, count: usize) -> Self {
        let target = span / count.max(1) as f64;
        if !(target.is_finite() && target > 0.0) {
            return Self::Seconds(DAY);
        }
        if target < 0.75 {
            return Self::Fraction(tick_step(0.0, span, count).min(0.5));
        }
        let candidates = [
            (1.0, Self::Seconds(1.0)),
            (5.0, Self::Seconds(5.0)),
            (15.0, Self::Seconds(15.0)),
            (30.0, Self::Seconds(30.0)),
            (MINUTE, Self::Seconds(MINUTE)),
            (5.0 * MINUTE, Self::Seconds(5.0 * MINUTE)),
            (15.0 * MINUTE, Self::Seconds(15.0 * MINUTE)),
            (30.0 * MINUTE, Self::Seconds(30.0 * MINUTE)),
            (HOUR, Self::Seconds(HOUR)),
            (3.0 * HOUR, Self::Seconds(3.0 * HOUR)),
            (6.0 * HOUR, Self::Seconds(6.0 * HOUR)),
            (12.0 * HOUR, Self::Seconds(12.0 * HOUR)),
            (DAY, Self::Seconds(DAY)),
            (2.0 * DAY, Self::Seconds(2.0 * DAY)),
            (7.0 * DAY, Self::Week),
            (MONTH, Self::Months(1)),
            (3.0 * MONTH, Self::Months(3)),
            (6.0 * MONTH, Self::Months(6)),
            (YEAR, Self::Years(1.0)),
        ];
        let distance = |c: f64| (target / c).ln().abs();
        let (size, best) = candidates
            .iter()
            .copied()
            .min_by(|a, b| distance(a.0).total_cmp(&distance(b.0)))
            .expect("candidates");
        if size == YEAR && target > YEAR {
            return Self::Years(tick_step(0.0, span / YEAR, count).max(1.0).round());
        }
        best
    }

    /// The ticks in `[lo, hi]`.
    fn ticks(&self, lo: f64, hi: f64) -> Vec<f64> {
        let within = |t: &f64| *t >= lo && *t <= hi;
        match *self {
            Self::Fraction(step) | Self::Seconds(step) => {
                let (start, end) = ((lo / step - 1e-9).ceil(), (hi / step + 1e-9).floor());
                (0..=((end - start).max(-1.0) as i64))
                    .map(|i| step_value(start + i as f64, step))
                    .filter(within)
                    .collect()
            }
            Self::Week => {
                let mut t = self.ceil(lo);
                let mut out = Vec::new();
                while t <= hi {
                    out.push(t);
                    t += 7.0 * DAY;
                }
                out
            }
            Self::Months(n) => {
                let (mut y, mut m) = month_at_or_after(self.ceil(lo));
                let mut out = Vec::new();
                loop {
                    let t = start_of(y, m, 1);
                    if t > hi {
                        break out;
                    }
                    out.push(t);
                    m += n;
                    if m > 12 {
                        (y, m) = (y + 1, m - 12);
                    }
                }
            }
            Self::Years(step) => {
                let step = step as i64;
                let (y0, _, _) = civil(self.ceil(lo));
                let mut y = y0.div_euclid(step) * step;
                let mut out = Vec::new();
                loop {
                    let t = start_of(y, 1, 1);
                    if t > hi {
                        break out;
                    }
                    if t >= lo {
                        out.push(t);
                    }
                    y += step;
                }
            }
        }
    }

    /// The last tick at or before `t`.
    fn floor(&self, t: f64) -> f64 {
        match *self {
            Self::Fraction(step) | Self::Seconds(step) => (t / step).floor() * step,
            Self::Week => {
                // 1970-01-05 was a Monday.
                let monday = 4.0 * DAY;
                ((t - monday) / (7.0 * DAY)).floor() * 7.0 * DAY + monday
            }
            Self::Months(n) => {
                let (y, m, _) = civil(t);
                start_of(y, m - (m - 1) % n, 1)
            }
            Self::Years(step) => {
                let (y, _, _) = civil(t);
                let step = step as i64;
                start_of(y.div_euclid(step) * step, 1, 1)
            }
        }
    }

    /// The first tick at or after `t`.
    fn ceil(&self, t: f64) -> f64 {
        let f = self.floor(t);
        if f >= t {
            return f;
        }
        match *self {
            Self::Fraction(step) | Self::Seconds(step) => f + step,
            Self::Week => f + 7.0 * DAY,
            Self::Months(n) => {
                let (y, m, _) = civil(f);
                let m = m + n;
                if m > 12 {
                    start_of(y + 1, m - 12, 1)
                } else {
                    start_of(y, m, 1)
                }
            }
            Self::Years(step) => {
                let (y, _, _) = civil(f);
                start_of(y + step as i64, 1, 1)
            }
        }
    }
}

/// The year and month of the month starting at or after `t` (`t` is a
/// month start).
fn month_at_or_after(t: f64) -> (i64, u32) {
    let (y, m, d) = civil(t);
    if d == 1 && t == start_of(y, m, 1) {
        (y, m)
    } else if m == 12 {
        (y + 1, 1)
    } else {
        (y, m + 1)
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The label of tick `t`: the coarsest calendar unit it begins.
fn label(t: f64, unit: &Interval) -> String {
    let whole = t.floor();
    let frac = t - whole;
    let tolerance = match unit {
        Interval::Fraction(step) => step * 1e-3,
        _ => 1e-6,
    };
    if frac > tolerance && frac < 1.0 - tolerance {
        let Interval::Fraction(step) = unit else {
            unreachable!("only fractional intervals fall inside a second")
        };
        let decimals = (-step.log10()).ceil().max(1.0) as usize;
        let digits = format!("{frac:.decimals$}");
        return digits.trim_start_matches('0').to_string();
    }
    let t = t.round();
    let days = (t / DAY).floor();
    let secs = (t - days * DAY) as i64;
    let (hh, mm, ss) = (secs / 3600, secs / 60 % 60, secs % 60);
    let (y, m, d) = civil(t);
    if ss != 0 {
        format!(":{ss:02}")
    } else if mm != 0 || hh != 0 {
        format!("{hh:02}:{mm:02}")
    } else if d != 1 {
        format!("{} {d}", MONTHS[m as usize - 1])
    } else if m != 1 {
        MONTHS[m as usize - 1].to_string()
    } else {
        y.to_string()
    }
}

/// `(year, month, day)` of the UTC day holding `t` seconds since the
/// epoch (proleptic Gregorian).
fn civil(t: f64) -> (i64, u32, u32) {
    // Howard Hinnant's `civil_from_days`.
    let z = (t / DAY).floor() as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Seconds since the epoch at midnight UTC starting `y-m-d`.
fn start_of(y: i64, m: u32, d: u32) -> f64 {
    // Howard Hinnant's `days_from_civil`.
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as f64 * DAY
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unix seconds at midnight UTC on a date (checked against `date -u
    /// -d YYYY-MM-DD +%s` for the constants below).
    fn date(y: i64, m: u32, d: u32) -> f64 {
        start_of(y, m, d)
    }

    #[test]
    fn calendar_conversions_round_trip_and_match_known_dates() {
        assert_eq!(date(1970, 1, 1), 0.0);
        assert_eq!(date(2020, 1, 1), 1_577_836_800.0);
        assert_eq!(date(2021, 1, 1), 1_609_459_200.0);
        assert_eq!(date(2023, 3, 1), 1_677_628_800.0);
        assert_eq!(date(2024, 1, 1), 1_704_067_200.0);
        assert_eq!(date(2024, 2, 29), 1_709_164_800.0);
        assert_eq!(date(1969, 12, 31), -86_400.0);
        for (y, m, d) in [(2024, 2, 29), (2000, 2, 29), (1900, 3, 1), (1969, 7, 20)] {
            assert_eq!(civil(date(y, m, d)), (y, m, d));
            assert_eq!(civil(date(y, m, d) + DAY - 1.0), (y, m, d));
        }
    }

    fn time(d0: f64, d1: f64) -> Time {
        Time::new().domain(d0, d1)
    }

    /// Daily ticks across February in a leap year: Feb 29 exists, and
    /// March 1 is labelled as a month.
    #[test]
    fn daily_ticks_cross_a_leap_day() {
        let t = time(date(2024, 2, 25) + 3_600.0, date(2024, 3, 5)).ticks(8);
        assert_eq!(
            t.labels,
            [
                "Feb 26", "Feb 27", "Feb 28", "Feb 29", "Mar", "Mar 2", "Mar 3", "Mar 4", "Mar 5"
            ]
        );
        assert_eq!(t.values[3], date(2024, 2, 29));
        assert_eq!(t.values[4], date(2024, 3, 1));
        // 2023 has no Feb 29.
        let t = time(date(2023, 2, 25), date(2023, 3, 3)).ticks(8);
        assert_eq!(
            t.labels,
            [
                "Feb 25", "Feb 26", "Feb 27", "Feb 28", "Mar", "Mar 2", "Mar 3"
            ]
        );
    }

    /// Monthly ticks land on the 1st of months of 28, 30 and 31 days.
    #[test]
    fn monthly_ticks_land_on_the_first_of_each_month() {
        let t = time(date(2023, 1, 15), date(2023, 12, 20)).ticks(12);
        let want: Vec<f64> = (2..=12).map(|m| date(2023, m, 1)).collect();
        assert_eq!(t.values, want);
        assert_eq!(t.labels[0], "Feb");
        assert_eq!(t.labels[1], "Mar");
        assert_eq!(t.values[1] - t.values[0], 28.0 * DAY);
        assert_eq!(t.values[2] - t.values[1], 31.0 * DAY);
        assert_eq!(t.values[3] - t.values[2], 30.0 * DAY);
    }

    /// Across a year boundary the January tick is labelled with the year.
    #[test]
    fn year_boundaries_are_labelled_with_the_year() {
        let t = time(date(2020, 6, 1), date(2023, 6, 1)).ticks(5);
        assert_eq!(t.labels, ["Jul", "2021", "Jul", "2022", "Jul", "2023"]);
        assert_eq!(t.values[1], date(2021, 1, 1));
        let quarters = time(date(2022, 11, 5), date(2023, 10, 1)).ticks(4);
        assert_eq!(quarters.labels, ["2023", "Apr", "Jul", "Oct"]);
        // Decades of years: 1–2–5 steps of whole years.
        let t = time(date(1990, 3, 1), date(2031, 1, 1)).ticks(5);
        assert_eq!(t.labels, ["2000", "2010", "2020", "2030"]);
        let t = time(date(2019, 3, 1), date(2025, 2, 1)).ticks(5);
        assert_eq!(t.labels, ["2020", "2021", "2022", "2023", "2024", "2025"]);
    }

    #[test]
    fn hours_minutes_and_seconds() {
        let day = date(2023, 11, 14);
        let t = time(day - 2.0 * HOUR, day + 10.0 * HOUR).ticks(6);
        assert_eq!(t.labels, ["Nov 14", "03:00", "06:00", "09:00"]);
        let t = time(day + 3_600.0 * 9.0 + 50.0, day + 3_600.0 * 9.0 + 400.0).ticks(6);
        assert_eq!(
            t.labels,
            ["09:01", "09:02", "09:03", "09:04", "09:05", "09:06"]
        );
        let t = time(day + 61.0, day + 90.0).ticks(6);
        assert_eq!(t.labels, [":05", ":10", ":15", ":20", ":25", ":30"]);
        let weeks = time(date(2023, 11, 1), date(2023, 12, 31)).ticks(9);
        // Mondays.
        assert_eq!(weeks.labels[0], "Nov 6");
        assert_eq!(weeks.values[1] - weeks.values[0], 7.0 * DAY);
    }

    /// Below a second: 1–2–5 steps, labelled by the fraction; the whole
    /// second takes its own label. Deep zooms of Unix seconds work too.
    #[test]
    fn fractions_of_a_second() {
        let s = date(2023, 11, 14) + 40.0;
        let t = time(s - 0.25, s + 0.5).ticks(4);
        assert_eq!(t.labels, [".8", ":40", ".2", ".4"]);
        let t = time(1.7e9 - 5e-4, 1.7e9 + 5e-4).ticks(5);
        // 1.7e9 is 2023-11-14 22:13:20 UTC: a whole second.
        assert_eq!(t.labels, [".9996", ".9998", ":20", ".0002", ".0004"]);
    }

    #[test]
    fn nice_extends_to_calendar_units() {
        let mut s = Time::new();
        s.fit_domain((date(2021, 3, 14) + 5_000.0, date(2023, 9, 2)))
            .unwrap();
        s.nice();
        let (d0, d1) = s.current_domain().unwrap();
        assert_eq!((civil(d0), civil(d1)), ((2021, 1, 1), (2023, 10, 1)));
        assert_eq!((d0 % DAY, d1 % DAY), (0.0, 0.0));
        assert!(s.is_auto());
    }
}
