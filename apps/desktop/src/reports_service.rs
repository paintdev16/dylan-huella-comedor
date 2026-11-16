//! Capa de datos y consultas para Historial y Reportes.
//!
//! Diseñada con DTOs serializables y un contrato desacoplado (`ReportsDataSource`)
//! para poder conectarse en etapas posteriores a la API Axum sin acoplar la UI.

use chrono::{Datelike as _, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::mock_data::{Meal, MenuKind, MockData};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

impl SortDirection {
    pub fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            Self::Asc => "↑",
            Self::Desc => "↓",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConsumptionSortColumn {
    #[default]
    Date,
    Pensioner,
    Kind,
    Portions,
    Amount,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PaymentSortColumn {
    #[default]
    Date,
    Pensioner,
    Kind,
    Amount,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DebtSortColumn {
    #[default]
    Balance,
    Pensioner,
    Consumed,
    Paid,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DateRangePreset {
    #[default]
    All,
    Today,
    Last7Days,
    CurrentMonth,
}

impl DateRangePreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todas las fechas",
            Self::Today => "Hoy",
            Self::Last7Days => "Últimos 7 días",
            Self::CurrentMonth => "Este mes",
        }
    }

    pub fn bounds(self, reference: NaiveDate) -> (Option<NaiveDate>, Option<NaiveDate>) {
        match self {
            Self::All => (None, None),
            Self::Today => (Some(reference), Some(reference)),
            Self::Last7Days => (Some(reference - Duration::days(6)), Some(reference)),
            Self::CurrentMonth => {
                let start = NaiveDate::from_ymd_opt(reference.year(), reference.month(), 1);
                (start, Some(reference))
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ConsumptionTypeFilter {
    #[default]
    All,
    Breakfast,
    EconomicLunch,
    SpecialLunch,
    SpecialAny,
}

impl ConsumptionTypeFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todos los tipos",
            Self::Breakfast => "Desayunos",
            Self::EconomicLunch => "Alm. económico",
            Self::SpecialLunch => "Alm. especial",
            Self::SpecialAny => "Todos los especiales",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum PensionerStatusFilter {
    #[default]
    All,
    Active,
    Inactive,
}

impl PensionerStatusFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::Active => "Activos",
            Self::Inactive => "Inactivos",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportQueryFilter {
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub pensioner_id: Option<u32>,
    pub consumption_type: ConsumptionTypeFilter,
    pub status: PensionerStatusFilter,
    pub only_third_party: bool,
    pub search: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsumptionReportRow {
    pub date: NaiveDate,
    pub time: String,
    pub pensioner_id: u32,
    pub pensioner_name: String,
    pub pensioner_active: bool,
    pub meal_label: String,
    pub menu_kind_label: String,
    pub is_breakfast: bool,
    pub is_special: bool,
    pub detail: Option<String>,
    pub portions: u32,
    pub cents: i64,
    pub picked_up_by: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentReportRow {
    pub movement_id: String,
    pub date: NaiveDate,
    pub time: String,
    pub pensioner_id: u32,
    pub pensioner_name: String,
    pub pensioner_active: bool,
    pub kind_label: String,
    pub cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebtReportRow {
    pub pensioner_id: u32,
    pub pensioner_name: String,
    pub pensioner_active: bool,
    pub total_portions: u32,
    pub consumed_cents: i64,
    pub paid_cents: i64,
    pub balance_cents: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeriodSummaryRow {
    pub period_label: String,
    pub breakfast_portions: u32,
    pub economic_lunch_portions: u32,
    pub special_portions: u32,
    pub total_portions: u32,
    pub third_party_deliveries: usize,
    pub consumed_cents: i64,
    pub paid_cents: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportTotals {
    pub breakfast_attentions: usize,
    pub lunch_attentions: usize,
    pub total_attentions: usize,
    pub breakfast_portions: u32,
    pub economic_lunch_portions: u32,
    pub special_portions: u32,
    pub total_portions: u32,
    pub third_party_deliveries: usize,
    pub total_consumed_cents: i64,
    pub total_paid_cents: i64,
    pub total_debt_cents: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportsSnapshot {
    pub consumptions: Vec<ConsumptionReportRow>,
    pub payments: Vec<PaymentReportRow>,
    pub debts: Vec<DebtReportRow>,
    pub daily_summary: Vec<PeriodSummaryRow>,
    pub monthly_summary: Vec<PeriodSummaryRow>,
    pub totals: ReportTotals,
}

/// Contrato de lectura desacoplado para reportes (futuro cliente REST Axum).
pub trait ReportsDataSource {
    fn query_reports(&self, filter: &ReportQueryFilter) -> ReportsSnapshot;
}

pub struct MockReportsDataSource<'a> {
    data: &'a MockData,
}

impl<'a> MockReportsDataSource<'a> {
    pub fn new(data: &'a MockData) -> Self {
        Self { data }
    }
}

fn matches_date(date: NaiveDate, from: Option<NaiveDate>, to: Option<NaiveDate>) -> bool {
    if let Some(f) = from {
        if date < f {
            return false;
        }
    }
    if let Some(t) = to {
        if date > t {
            return false;
        }
    }
    true
}

fn matches_status(active: bool, status: PensionerStatusFilter) -> bool {
    match status {
        PensionerStatusFilter::All => true,
        PensionerStatusFilter::Active => active,
        PensionerStatusFilter::Inactive => !active,
    }
}

fn matches_search(id: u32, name: &str, extra: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    format!("#{id} {id} {name} {extra}").to_lowercase().contains(&q)
}

impl ReportsDataSource for MockReportsDataSource<'_> {
    fn query_reports(&self, filter: &ReportQueryFilter) -> ReportsSnapshot {
        let mut consumptions = Vec::new();
        let mut payments = Vec::new();
        let mut totals = ReportTotals::default();
        let mut daily_map: BTreeMap<NaiveDate, PeriodSummaryRow> = BTreeMap::new();
        let mut monthly_map: BTreeMap<(i32, u32), PeriodSummaryRow> = BTreeMap::new();

        for c in &self.data.consumptions {
            if !matches_date(c.date, filter.date_from, filter.date_to) {
                continue;
            }
            if filter.pensioner_id.is_some_and(|pid| pid != c.pensioner_id) {
                continue;
            }
            let Some(person) = self
                .data
                .pensioners
                .iter()
                .find(|p| p.id == c.pensioner_id)
            else {
                continue;
            };
            if !matches_status(person.active, filter.status) {
                continue;
            }
            if filter.only_third_party && c.picked_up_by.is_none() {
                continue;
            }
            let matches_type = match filter.consumption_type {
                ConsumptionTypeFilter::All => true,
                ConsumptionTypeFilter::Breakfast => c.meal == Meal::Breakfast,
                ConsumptionTypeFilter::EconomicLunch => {
                    c.meal == Meal::Lunch && c.menu_kind == MenuKind::Economic
                }
                ConsumptionTypeFilter::SpecialLunch => {
                    c.meal == Meal::Lunch && c.menu_kind == MenuKind::Special
                }
                ConsumptionTypeFilter::SpecialAny => c.menu_kind == MenuKind::Special,
            };
            if !matches_type {
                continue;
            }
            let p_name = person.name();
            let extra = format!(
                "{} {}",
                c.detail.as_deref().unwrap_or(""),
                c.picked_up_by.as_deref().unwrap_or("")
            );
            if !matches_search(person.id, &p_name, &extra, &filter.search) {
                continue;
            }

            let is_breakfast = c.meal == Meal::Breakfast;
            let is_special = c.menu_kind == MenuKind::Special;

            totals.total_attentions += 1;
            if is_breakfast {
                totals.breakfast_attentions += 1;
            } else {
                totals.lunch_attentions += 1;
            }
            totals.total_portions += c.portions;
            totals.total_consumed_cents += c.cents;
            if c.picked_up_by.is_some() {
                totals.third_party_deliveries += 1;
            }
            if is_special {
                totals.special_portions += c.portions;
            } else if is_breakfast {
                totals.breakfast_portions += c.portions;
            } else {
                totals.economic_lunch_portions += c.portions;
            }

            let d_entry = daily_map.entry(c.date).or_insert_with(|| PeriodSummaryRow {
                period_label: c.date.format("%d/%m/%Y").to_string(),
                ..Default::default()
            });
            let m_entry = monthly_map
                .entry((c.date.year(), c.date.month()))
                .or_insert_with(|| PeriodSummaryRow {
                    period_label: format!("{:02}/{}", c.date.month(), c.date.year()),
                    ..Default::default()
                });

            for entry in [d_entry, m_entry] {
                entry.total_portions += c.portions;
                entry.consumed_cents += c.cents;
                if c.picked_up_by.is_some() {
                    entry.third_party_deliveries += 1;
                }
                if is_special {
                    entry.special_portions += c.portions;
                } else if is_breakfast {
                    entry.breakfast_portions += c.portions;
                } else {
                    entry.economic_lunch_portions += c.portions;
                }
            }

            consumptions.push(ConsumptionReportRow {
                date: c.date,
                time: c.time.to_string(),
                pensioner_id: person.id,
                pensioner_name: p_name,
                pensioner_active: person.active,
                meal_label: c.meal.label().to_string(),
                menu_kind_label: c.menu_kind.label().to_string(),
                is_breakfast,
                is_special,
                detail: c.detail.clone(),
                portions: c.portions,
                cents: c.cents,
                picked_up_by: c.picked_up_by.clone(),
            });
        }

        for p in &self.data.payments {
            if !matches_date(p.date, filter.date_from, filter.date_to) {
                continue;
            }
            if filter.pensioner_id.is_some_and(|pid| pid != p.pensioner_id) {
                continue;
            }
            let Some(person) = self
                .data
                .pensioners
                .iter()
                .find(|pe| pe.id == p.pensioner_id)
            else {
                continue;
            };
            if !matches_status(person.active, filter.status) {
                continue;
            }
            let p_name = person.name();
            if !matches_search(person.id, &p_name, &p.movement_id, &filter.search) {
                continue;
            }

            totals.total_paid_cents += p.cents;
            daily_map
                .entry(p.date)
                .or_insert_with(|| PeriodSummaryRow {
                    period_label: p.date.format("%d/%m/%Y").to_string(),
                    ..Default::default()
                })
                .paid_cents += p.cents;
            monthly_map
                .entry((p.date.year(), p.date.month()))
                .or_insert_with(|| PeriodSummaryRow {
                    period_label: format!("{:02}/{}", p.date.month(), p.date.year()),
                    ..Default::default()
                })
                .paid_cents += p.cents;

            payments.push(PaymentReportRow {
                movement_id: p.movement_id.clone(),
                date: p.date,
                time: p.time.to_string(),
                pensioner_id: person.id,
                pensioner_name: p_name,
                pensioner_active: person.active,
                kind_label: p.kind.label().to_string(),
                cents: p.cents,
            });
        }

        let mut debts = Vec::new();
        for person in &self.data.pensioners {
            if filter.pensioner_id.is_some_and(|pid| pid != person.id) {
                continue;
            }
            if !matches_status(person.active, filter.status) {
                continue;
            }
            let p_name = person.name();
            if !matches_search(person.id, &p_name, "", &filter.search) {
                continue;
            }
            let consumed = self.data.consumed_total(person.id);
            let paid = self.data.paid_total(person.id);
            let balance = self.data.balance(person.id);
            let portions: u32 = self
                .data
                .consumptions
                .iter()
                .filter(|c| c.pensioner_id == person.id)
                .map(|c| c.portions)
                .sum();
            totals.total_debt_cents += balance;
            debts.push(DebtReportRow {
                pensioner_id: person.id,
                pensioner_name: p_name,
                pensioner_active: person.active,
                total_portions: portions,
                consumed_cents: consumed,
                paid_cents: paid,
                balance_cents: balance,
            });
        }

        ReportsSnapshot {
            consumptions,
            payments,
            debts,
            daily_summary: daily_map.into_values().rev().collect(),
            monthly_summary: monthly_map.into_values().rev().collect(),
            totals,
        }
    }
}

pub fn paginate<T>(items: &[T], page: usize, page_size: usize) -> (&[T], usize, usize) {
    let size = page_size.max(1);
    let total_pages = items.len().div_ceil(size).max(1);
    let clamped_page = page.min(total_pages - 1);
    let start = clamped_page * size;
    let end = (start + size).min(items.len());
    (&items[start..end], clamped_page, total_pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data::DraftConsumptionItem;
    use std::collections::HashSet;

    #[test]
    fn reports_totals_differentiate_meals_third_party_and_paginate() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(today);
        let mut tokens = HashSet::new();
        data.record_consumptions(
            today,
            "13:20",
            1,
            &[DraftConsumptionItem {
                id: 1,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Special,
                description: "Lomo saltado".into(),
                portions: 2,
                unit_price_cents: 1500,
            }],
            Some("Familiar"),
            1,
            &mut tokens,
        )
        .unwrap();

        let source = MockReportsDataSource::new(&data);
        let snap = source.query_reports(&ReportQueryFilter::default());
        assert_eq!(snap.totals.breakfast_portions, 3);
        assert_eq!(snap.totals.economic_lunch_portions, 4);
        assert_eq!(snap.totals.special_portions, 2);
        assert_eq!(snap.totals.total_portions, 9);
        assert_eq!(snap.totals.third_party_deliveries, 1);
        assert_eq!(snap.totals.total_consumed_cents, 4200 + 900 + 3000);

        let (slice, page, total_pages) = paginate(&snap.consumptions, 5, 4);
        assert_eq!(total_pages, 2);
        assert_eq!(page, 1);
        assert_eq!(slice.len(), 2);

        let special_only = source.query_reports(&ReportQueryFilter {
            consumption_type: ConsumptionTypeFilter::SpecialLunch,
            only_third_party: true,
            ..Default::default()
        });
        assert_eq!(special_only.consumptions.len(), 1);
        assert_eq!(special_only.totals.special_portions, 2);
    }

    #[test]
    fn dashboard_cash_and_reports_stay_consistent_after_mutations() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(today);
        let mut c_tokens = HashSet::new();
        let mut p_tokens = HashSet::new();

        // 1. Registrar nuevo pensionista
        let new_pid = data
            .save_pensioner(None, "Sofía", "Mendoza", "987654321")
            .unwrap();

        // 2. Registrar consumo con 3 raciones en 1 atención de desayuno
        data.record_consumptions(
            today,
            "08:45",
            new_pid,
            &[DraftConsumptionItem {
                id: 1,
                meal: Meal::Breakfast,
                menu_kind: MenuKind::Economic,
                description: "Desayuno".into(),
                portions: 3,
                unit_price_cents: 500,
            }],
            None,
            10,
            &mut c_tokens,
        )
        .unwrap();

        // 3. Registrar pago parcial en Caja
        data.record_payment(today, "09:00", new_pid, 500, 10, &mut p_tokens)
            .unwrap();

        // Dashboard (fecha hoy)
        let dash = data.summary(today);
        assert_eq!(dash.registered, 5);
        assert_eq!(dash.active, 4);
        // 2 desayunos demo + 1 atención nueva = 3 atenciones de desayuno; 3 + 3 = 6 raciones
        assert_eq!(dash.breakfasts, 3);
        assert_eq!(dash.breakfast_portions, 6);
        assert_eq!(dash.attentions(), 5);
        assert_eq!(dash.portions, 9);

        // Caja: suma de saldos por pensionista
        let cash_total_debt: i64 = data.pensioners.iter().map(|p| data.balance(p.id)).sum();
        assert_eq!(dash.debt_cents, cash_total_debt);

        // Reportes con filtro Hoy
        let (date_from, date_to) = DateRangePreset::Today.bounds(today);
        let snap_today = MockReportsDataSource::new(&data).query_reports(&ReportQueryFilter {
            date_from,
            date_to,
            ..Default::default()
        });

        assert_eq!(snap_today.totals.total_attentions, dash.attentions());
        assert_eq!(snap_today.totals.breakfast_attentions, dash.breakfasts);
        assert_eq!(snap_today.totals.lunch_attentions, dash.lunches);
        assert_eq!(snap_today.totals.total_portions, dash.portions);
        assert_eq!(snap_today.totals.total_consumed_cents, dash.consumed_cents);
        assert_eq!(snap_today.totals.total_paid_cents, dash.paid_cents);
        assert_eq!(snap_today.totals.total_debt_cents, cash_total_debt);
    }
}
