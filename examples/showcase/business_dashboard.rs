// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Business Dashboard - Showcase Example
//!
//! This example demonstrates a real-world business intelligence dashboard
//! with multiple metrics, KPIs, and trend analysis.
//!
//! ## What You'll Learn
//! - Creating professional business visualizations
//! - Combining multiple data series
//! - KPI tracking and presentation
//! - Dashboard-style data organization
//!
//! Run with: `cargo run --example business_dashboard`

use gup::prelude::*;
use std::sync::Arc;

// Business metrics data
#[derive(Debug, Clone)]
struct MonthlyMetrics {
    month: String,
    month_index: f32,
    revenue: f32,
    expenses: f32,
    customers: f32,
    conversion_rate: f32,
}

impl MonthlyMetrics {
    fn new(
        month: &str,
        month_index: f32,
        revenue: f32,
        expenses: f32,
        customers: f32,
        conversion_rate: f32,
    ) -> Self {
        Self {
            month: month.to_string(),
            month_index,
            revenue,
            expenses,
            customers,
            conversion_rate,
        }
    }

    fn profit(&self) -> f32 {
        self.revenue - self.expenses
    }

    fn profit_margin(&self) -> f32 {
        (self.profit() / self.revenue) * 100.0
    }
}

// Generate realistic business dashboard data
fn generate_dashboard_data() -> Vec<MonthlyMetrics> {
    vec![
        MonthlyMetrics::new("Jan", 1.0, 125000.0, 95000.0, 1250.0, 0.032),
        MonthlyMetrics::new("Feb", 2.0, 132000.0, 98000.0, 1320.0, 0.034),
        MonthlyMetrics::new("Mar", 3.0, 148000.0, 105000.0, 1480.0, 0.038),
        MonthlyMetrics::new("Apr", 4.0, 156000.0, 108000.0, 1560.0, 0.041),
        MonthlyMetrics::new("May", 5.0, 172000.0, 115000.0, 1720.0, 0.045),
        MonthlyMetrics::new("Jun", 6.0, 185000.0, 120000.0, 1850.0, 0.048),
        MonthlyMetrics::new("Jul", 7.0, 198000.0, 125000.0, 1980.0, 0.051),
        MonthlyMetrics::new("Aug", 8.0, 205000.0, 128000.0, 2050.0, 0.053),
        MonthlyMetrics::new("Sep", 9.0, 195000.0, 122000.0, 1950.0, 0.050),
        MonthlyMetrics::new("Oct", 10.0, 210000.0, 130000.0, 2100.0, 0.055),
        MonthlyMetrics::new("Nov", 11.0, 225000.0, 135000.0, 2250.0, 0.058),
        MonthlyMetrics::new("Dec", 12.0, 245000.0, 140000.0, 2450.0, 0.062),
    ]
}

// Calculate KPIs
struct DashboardKPIs {
    total_revenue: f32,
    total_profit: f32,
    avg_monthly_revenue: f32,
    avg_profit_margin: f32,
    total_customers: f32,
    avg_conversion_rate: f32,
    revenue_growth: f32,
    customer_growth: f32,
}

impl DashboardKPIs {
    fn from_data(data: &[MonthlyMetrics]) -> Self {
        let total_revenue: f32 = data.iter().map(|m| m.revenue).sum();
        let total_profit: f32 = data.iter().map(|m| m.profit()).sum();
        let total_customers: f32 = data.iter().map(|m| m.customers).sum();
        let avg_conversion_rate: f32 =
            data.iter().map(|m| m.conversion_rate).sum::<f32>() / data.len() as f32;

        let first_revenue = data.first().unwrap().revenue;
        let last_revenue = data.last().unwrap().revenue;
        let revenue_growth = ((last_revenue - first_revenue) / first_revenue) * 100.0;

        let first_customers = data.first().unwrap().customers;
        let last_customers = data.last().unwrap().customers;
        let customer_growth = ((last_customers - first_customers) / first_customers) * 100.0;

        Self {
            total_revenue,
            total_profit,
            avg_monthly_revenue: total_revenue / data.len() as f32,
            avg_profit_margin: (total_profit / total_revenue) * 100.0,
            total_customers,
            avg_conversion_rate: avg_conversion_rate * 100.0,
            revenue_growth,
            customer_growth,
        }
    }

    fn print_dashboard(&self) {
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║           BUSINESS DASHBOARD - ANNUAL OVERVIEW          ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║                                                          ║");
        println!("║  KEY PERFORMANCE INDICATORS                              ║");
        println!("║                                                          ║");
        println!(
            "║  💰 Total Revenue:        ${:>12.0}                  ║",
            self.total_revenue
        );
        println!(
            "║  📈 Total Profit:         ${:>12.0}                  ║",
            self.total_profit
        );
        println!(
            "║  📊 Avg Monthly Revenue:  ${:>12.0}                  ║",
            self.avg_monthly_revenue
        );
        println!(
            "║  💹 Avg Profit Margin:    {:>11.1}%                   ║",
            self.avg_profit_margin
        );
        println!("║                                                          ║");
        println!("║  GROWTH METRICS                                          ║");
        println!("║                                                          ║");
        println!(
            "║  📈 Revenue Growth:       {:>11.1}% YoY               ║",
            self.revenue_growth
        );
        println!(
            "║  👥 Customer Growth:      {:>11.1}% YoY               ║",
            self.customer_growth
        );
        println!("║                                                          ║");
        println!("║  CUSTOMER METRICS                                        ║");
        println!("║                                                          ║");
        println!(
            "║  👥 Total Customers:      {:>12.0}                  ║",
            self.total_customers
        );
        println!(
            "║  🎯 Avg Conversion Rate:  {:>11.2}%                   ║",
            self.avg_conversion_rate
        );
        println!("║                                                          ║");
        println!("╚══════════════════════════════════════════════════════════╝");
    }
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("=== Business Dashboard Example ===");
    println!();

    // Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("GPU context initialized");
    println!();

    // Generate dashboard data
    let data = generate_dashboard_data();

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let mut chart = line()
            .x(AccessorFunction::new(|m: &MonthlyMetrics| {
                AccessorValue::Float(m.month_index)
            }))
            .y(AccessorFunction::new(|m: &MonthlyMetrics| {
                AccessorValue::Float(m.revenue)
            }))
            .stroke_color([0.2, 0.7, 0.3, 1.0])
            .stroke_width_px(2.5)
            .show_grid(true)
            .show_axes(true)
            .title("Revenue Trend")
            .build_with_data(data, context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    println!("Generated 12 months of business metrics");
    println!();

    // Calculate and display KPIs
    let kpis = DashboardKPIs::from_data(&data);
    kpis.print_dashboard();
    println!();

    // ========================================
    // Chart 1: Revenue vs. Expenses
    // ========================================
    println!("--- Chart 1: Revenue & Expenses Trend ---");

    let revenue_chart = line()
        .x(AccessorFunction::new(|m: &MonthlyMetrics| {
            AccessorValue::Float(m.month_index)
        }))
        .y(AccessorFunction::new(|m: &MonthlyMetrics| {
            AccessorValue::Float(m.revenue)
        }))
        .stroke_color([0.2, 0.7, 0.3, 1.0]) // Green for revenue
        .stroke_width_px(2.5)
        .linear();

    let expenses_chart = line()
        .x(AccessorFunction::new(|m: &MonthlyMetrics| {
            AccessorValue::Float(m.month_index)
        }))
        .y(AccessorFunction::new(|m: &MonthlyMetrics| {
            AccessorValue::Float(m.expenses)
        }))
        .stroke_color([0.9, 0.4, 0.2, 1.0]) // Orange for expenses
        .stroke_width_px(2.5)
        .linear();

    let revenue_selection = revenue_chart.build_with_data(data.clone(), context.clone())?;
    let expenses_selection = expenses_chart.build_with_data(data.clone(), context.clone())?;

    println!("  Revenue line: {} points", revenue_selection.len());
    println!("  Expenses line: {} points", expenses_selection.len());
    println!();

    // ========================================
    // Chart 2: Customer Acquisition
    // ========================================
    println!("--- Chart 2: Customer Growth ---");

    let customers_chart = bar()
        .x(AccessorFunction::new(|m: &MonthlyMetrics| {
            AccessorValue::Float(m.month_index)
        }))
        .y(AccessorFunction::new(|m: &MonthlyMetrics| {
            AccessorValue::Float(m.customers)
        }))
        .fill(AccessorFunction::new(|_: &MonthlyMetrics| {
            AccessorValue::Color([0.2, 0.6, 0.9, 1.0])
        })) // Blue
        .vertical();

    let customers_selection = customers_chart.build_with_data(data.clone(), context)?;
    println!("  Customer bars: {}", customers_selection.len());
    println!();

    // ========================================
    // Month-by-Month Analysis
    // ========================================
    println!("--- Monthly Performance Details ---");
    println!();
    println!(" Month  | Revenue  | Expenses | Profit   | Margin  | Customers | Conv. Rate");
    println!("--------|----------|----------|----------|---------|-----------|------------");

    for metrics in &data {
        println!(
            " {:6} | ${:>7.0} | ${:>7.0} | ${:>7.0} | {:>5.1}% | {:>9.0} | {:>9.2}%",
            metrics.month,
            metrics.revenue,
            metrics.expenses,
            metrics.profit(),
            metrics.profit_margin(),
            metrics.customers,
            metrics.conversion_rate * 100.0
        );
    }
    println!();

    println!("Dashboard Summary:");
    println!("  ✓ Generated comprehensive business metrics");
    println!("  ✓ Created professional KPI dashboard");
    println!("  ✓ Built multi-chart visualization suite");
    println!("  ✓ Calculated growth and performance indicators");
    println!();

    println!("This showcase demonstrates:");
    println!("  • Real-world business intelligence visualization");
    println!("  • Professional dashboard layout and KPIs");
    println!("  • Multiple chart types working together");
    println!("  • Detailed trend and performance analysis");
    println!("  • Production-ready data presentation");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dashboard_data() {
        let data = generate_dashboard_data();
        assert_eq!(data.len(), 12); // 12 months

        // Check that revenue increases over time (with minor fluctuations)
        let first_revenue = data.first().unwrap().revenue;
        let last_revenue = data.last().unwrap().revenue;
        assert!(last_revenue > first_revenue);
    }

    #[test]
    fn test_profit_calculation() {
        let metrics = MonthlyMetrics::new("Jan", 1.0, 100000.0, 70000.0, 1000.0, 0.03);
        assert!((metrics.profit() - 30000.0).abs() < 0.01);
        assert!((metrics.profit_margin() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_kpis_calculation() {
        let data = generate_dashboard_data();
        let kpis = DashboardKPIs::from_data(&data);

        assert!(kpis.total_revenue > 0.0);
        assert!(kpis.total_profit > 0.0);
        assert!(kpis.avg_profit_margin > 0.0 && kpis.avg_profit_margin < 100.0);
        assert!(kpis.revenue_growth > 0.0); // Should show growth
    }

    #[tokio::test]
    async fn test_revenue_chart_creation() {
        let data = generate_dashboard_data();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = line()
            .x(AccessorFunction::new(|m: &MonthlyMetrics| {
                AccessorValue::Float(m.month_index)
            }))
            .y(AccessorFunction::new(|m: &MonthlyMetrics| {
                AccessorValue::Float(m.revenue)
            }));

        let selection = chart.build_with_data(data, context).unwrap();
        assert_eq!(selection.len(), 12);
    }
}
