use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use serde::Serialize;

use crate::data::EraPaid;

const DAYS_PER_YEAR: f64 = 365.25;

/// Pre-2026-03-14: 120M DOT/year
const OLD_DAILY_ISSUANCE: f64 = 120_000_000.0 / DAYS_PER_YEAR;

/// Hard cap target: 21B DOT
const TARGET_TOTAL_ISSUANCE: f64 = 2_100_000_000.0;

/// Approximate total issuance at March 14, 2026 (to be updated with real value)
const MARCH_2026_TI: f64 = 1_670_000_000.0;

/// 26.28% closer to target every 2-year step (Perbill::from_parts(262_800_000))
const BI_ANNUAL_RATE: f64 = 0.2628;

/// Step period: 2 years in days
const STEP_PERIOD_DAYS: f64 = 2.0 * DAYS_PER_YEAR;

/// The date the new issuance model takes effect
fn new_model_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 14).unwrap()
}

/// Evaluate the stepped curve total issuance at a given number of periods.
///
/// Formula: target - (target - initial) * (1 - rate)^num_periods
///
/// Ported from polkadot-sdk `SteppedCurve::evaluate` with `RemainingPct`.
fn stepped_curve_ti(num_periods: u32) -> f64 {
    let diff = TARGET_TOTAL_ISSUANCE - MARCH_2026_TI;
    let scale = (1.0 - BI_ANNUAL_RATE).powi(num_periods as i32);
    TARGET_TOTAL_ISSUANCE - diff * scale
}

fn daily_issuance_for(date: NaiveDateTime) -> f64 {
    if date.date() >= new_model_date() {
        // Curve start is 2 years before March 14, 2026 so that the first step
        // lands exactly on that date.
        let curve_start = new_model_date()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            - TimeDelta::days(STEP_PERIOD_DAYS as i64);
        let days_since_start = (date - curve_start).num_days().max(0) as f64;
        let num_periods = (days_since_start / STEP_PERIOD_DAYS).floor() as u32;

        // Emission for this 2-year step period
        let ti_now = stepped_curve_ti(num_periods);
        let ti_prev = if num_periods > 0 {
            stepped_curve_ti(num_periods - 1)
        } else {
            MARCH_2026_TI
        };
        let step_emission = ti_now - ti_prev;

        // Spread evenly across the 2-year period
        step_emission / STEP_PERIOD_DAYS
    } else {
        OLD_DAILY_ISSUANCE
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartPoint {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Annualized inflation rate (%)
    pub inflation_rate: f64,
    /// Total supply in DOT
    pub supply: f64,
    /// Daily issuance in DOT
    pub daily_issuance: f64,
    /// Whether this is a projected (future) point
    pub is_projected: bool,
}

#[derive(Debug, Serialize)]
pub struct ChartData {
    pub points: Vec<ChartPoint>,
    pub y_min_inflation: f64,
    pub y_max_inflation: f64,
    pub y_min_supply: f64,
    pub y_max_supply: f64,
}

/// Build historical + projected chart data from EraPaid events.
pub fn compute_chart_data(events: &[EraPaid], projection_years: f64) -> ChartData {
    let mut points = Vec::new();

    // -- Historical points --
    for event in events {
        let daily_increase = event.amount_paid_dot();
        let issuance = event.total_issuance_dot();
        let inflation_rate = (daily_increase / issuance) * DAYS_PER_YEAR * 100.0;

        points.push(ChartPoint {
            timestamp: event.timestamp.clone(),
            inflation_rate,
            supply: issuance,
            daily_issuance: daily_increase,
            is_projected: false,
        });
    }

    // -- Projections --
    if let Some(last) = points.last().cloned() {
        let projection_days = (projection_years * DAYS_PER_YEAR) as i64;
        let base_ts = parse_timestamp(&last.timestamp);

        let mut supply = last.supply;

        for day in 1..=projection_days {
            let ts = base_ts + TimeDelta::days(day);
            let daily = daily_issuance_for(ts);
            supply += daily;
            let inflation_rate = (daily * DAYS_PER_YEAR / supply) * 100.0;

            // Downsample: keep every 30th day
            if day % 30 == 0 || day == projection_days {
                points.push(ChartPoint {
                    timestamp: ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    inflation_rate,
                    supply,
                    daily_issuance: daily,
                    is_projected: true,
                });
            }
        }
    }

    // Compute axis bounds
    let (mut min_inf, mut max_inf) = (f64::MAX, f64::MIN);
    let (mut min_sup, mut max_sup) = (f64::MAX, f64::MIN);
    for p in &points {
        min_inf = min_inf.min(p.inflation_rate);
        max_inf = max_inf.max(p.inflation_rate);
        min_sup = min_sup.min(p.supply);
        max_sup = max_sup.max(p.supply);
    }

    ChartData {
        points,
        y_min_inflation: min_inf,
        y_max_inflation: max_inf,
        y_min_supply: min_sup,
        y_max_supply: max_sup,
    }
}

fn parse_timestamp(ts: &str) -> NaiveDateTime {
    // Try common formats from Subsquid
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.fZ")
        .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f"))
        .unwrap_or_default()
}
