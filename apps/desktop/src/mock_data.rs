//! Datos de demostración compartidos por las vistas del escritorio.
use chrono::{Duration, Local, NaiveDate};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Pensioner {
    pub id: u32,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub active: bool,
    pub fingerprint_registered: bool,
    pub biometric_id: Option<String>,
}
impl Pensioner {
    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
    pub fn biometric_status(&self) -> &'static str {
        if !self.active {
            "Inactivo"
        } else if self.fingerprint_registered {
            "Registrado"
        } else {
            "Pendiente"
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meal {
    Breakfast,
    Lunch,
}
impl Meal {
    pub fn label(self) -> &'static str {
        match self {
            Self::Breakfast => "Desayuno",
            Self::Lunch => "Almuerzo",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MenuKind {
    #[default]
    Economic,
    Special,
}

impl MenuKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Economic => "Económico",
            Self::Special => "Especial",
        }
    }
}

/// Precios predeterminados configurables en céntimos.
/// Valor inicial de prueba para ración económica: S/ 7.00 (700 céntimos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsumptionPricing {
    pub economic_breakfast_cents: i64,
    pub economic_lunch_cents: i64,
    pub special_default_cents: i64,
}

impl Default for ConsumptionPricing {
    fn default() -> Self {
        Self {
            economic_breakfast_cents: 700,
            economic_lunch_cents: 700,
            special_default_cents: 1200,
        }
    }
}

impl ConsumptionPricing {
    pub fn economic_for(&self, meal: Meal) -> i64 {
        match meal {
            Meal::Breakfast => self.economic_breakfast_cents,
            Meal::Lunch => self.economic_lunch_cents,
        }
    }
}

/// Línea de borrador antes de confirmar una atención.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftConsumptionItem {
    pub id: u64,
    pub meal: Meal,
    pub menu_kind: MenuKind,
    pub description: String,
    pub portions: u32,
    pub unit_price_cents: i64,
}

impl DraftConsumptionItem {
    pub fn subtotal_cents(&self) -> i64 {
        self.unit_price_cents.saturating_mul(self.portions as i64)
    }
}

/// Parsea un precio en soles (ej. `"7"`, `"7.5"`, `"12.50"`) a céntimos.
pub fn parse_price_cents(raw: &str) -> Result<i64, String> {
    let cleaned = raw.trim().trim_start_matches("S/").trim().replace(',', ".");
    if cleaned.is_empty() {
        return Err("Ingresa un precio válido.".into());
    }
    let mut parts = cleaned.split('.');
    let whole_str = parts.next().unwrap_or("");
    let frac_str = parts.next();
    if parts.next().is_some() {
        return Err("Formato de precio inválido (usa por ejemplo 7.00).".into());
    }
    if whole_str.is_empty() || !whole_str.chars().all(|c| c.is_ascii_digit()) {
        return Err("El precio debe ser un número positivo.".into());
    }
    let whole: i64 = whole_str
        .parse()
        .map_err(|_| "El monto ingresado es demasiado grande.".to_string())?;
    let frac: i64 = match frac_str {
        None => 0,
        Some(f) if f.is_empty() => 0,
        Some(f) if f.len() == 1 && f.chars().all(|c| c.is_ascii_digit()) => {
            f.parse::<i64>().unwrap_or(0) * 10
        }
        Some(f) if f.len() == 2 && f.chars().all(|c| c.is_ascii_digit()) => {
            f.parse::<i64>().unwrap_or(0)
        }
        _ => return Err("El precio admite como máximo 2 decimales.".into()),
    };
    let cents = whole
        .checked_mul(100)
        .and_then(|v| v.checked_add(frac))
        .ok_or_else(|| "El monto ingresado es demasiado grande.".to_string())?;
    if cents <= 0 {
        return Err("El precio debe ser mayor a S/ 0.00.".into());
    }
    Ok(cents)
}

#[derive(Clone, Debug)]
pub struct Consumption {
    pub date: NaiveDate,
    pub time: String,
    pub pensioner_id: u32,
    pub meal: Meal,
    pub portions: u32,
    pub cents: i64,
    pub menu_kind: MenuKind,
    pub detail: Option<String>,
    pub picked_up_by: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaymentKind {
    CashPayment,
    TicketCredit { count: u32, unit_cents: i64 },
}

impl PaymentKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CashPayment => "Pago",
            Self::TicketCredit { .. } => "Abono por tickets",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Payment {
    pub date: NaiveDate,
    pub time: String,
    pub pensioner_id: u32,
    pub cents: i64,
    pub movement_id: String,
    pub kind: PaymentKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CashMovementKind {
    Opening,
    Closing,
    Payment,
    TicketCredit,
}

impl CashMovementKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Opening => "Apertura de caja",
            Self::Closing => "Cierre de caja",
            Self::Payment => "Pago en efectivo",
            Self::TicketCredit => "Abono por tickets",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CashMovement {
    pub id: String,
    pub date: NaiveDate,
    pub time: String,
    pub kind: CashMovementKind,
    #[allow(dead_code)]
    pub pensioner_id: Option<u32>,
    pub description: String,
    pub cents: i64,
    pub operator_name: String,
    pub ticket_count: Option<u32>,
    pub ticket_unit_cents: Option<i64>,
    pub consumption_cents: Option<i64>,
    pub resulting_balance_cents: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct CashRegisterState {
    pub is_open: bool,
    pub opening_cents: i64,
    /// Valor inicial configurable del ticket físico: S/ 7.00 (700 céntimos).
    pub ticket_value_cents: i64,
    pub next_movement_seq: u32,
    pub movements: Vec<CashMovement>,
}

impl Default for CashRegisterState {
    fn default() -> Self {
        Self {
            is_open: true,
            opening_cents: 10000,
            ticket_value_cents: 700,
            next_movement_seq: 2,
            movements: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonthlyPensionerSummary {
    pub month_label: String,
    pub consumed_cents: i64,
    pub cash_paid_cents: i64,
    pub tickets_cents: i64,
    pub net_balance_cents: i64,
}

#[derive(Clone, Debug, Default)]
pub struct MockData {
    pub pensioners: Vec<Pensioner>,
    pub consumptions: Vec<Consumption>,
    pub payments: Vec<Payment>,
    pub cash_register: CashRegisterState,
}

#[derive(Clone, Debug)]
pub struct Activity {
    pub time: String,
    pub person: String,
    pub kind: &'static str,
    pub portions: Option<u32>,
    pub cents: i64,
}
#[derive(Clone, Debug, Default)]
pub struct DashboardSummary {
    pub registered: usize,
    pub active: usize,
    pub breakfasts: usize,
    pub lunches: usize,
    pub breakfast_portions: u32,
    pub lunch_portions: u32,
    pub portions: u32,
    pub consumed_cents: i64,
    pub paid_cents: i64,
    pub cash_paid_cents: i64,
    pub tickets_count: u32,
    pub tickets_cents: i64,
    pub debt_cents: i64,
    pub credit_balance_cents: i64,
    pub activity: Vec<Activity>,
}

impl DashboardSummary {
    /// Total de atenciones (transacciones de consumo) registradas en la fecha.
    pub fn attentions(&self) -> usize {
        self.breakfasts + self.lunches
    }
}
impl MockData {
    pub fn save_pensioner(
        &mut self,
        id: Option<u32>,
        first: &str,
        last: &str,
        phone: &str,
    ) -> Result<u32, String> {
        let (first, last, phone) = (first.trim(), last.trim(), phone.trim());
        if first.is_empty() || last.is_empty() {
            return Err("Ingresa nombres y apellidos.".into());
        }
        if first.chars().count() > 80 || last.chars().count() > 80 {
            return Err("Nombres y apellidos admiten hasta 80 caracteres cada uno.".into());
        }
        if !phone.is_empty()
            && (phone.len() < 7 || phone.len() > 15 || !phone.chars().all(|c| c.is_ascii_digit()))
        {
            return Err("El celular debe contener entre 7 y 15 dígitos.".into());
        }
        if let Some(id) = id {
            let person = self
                .pensioners
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("Pensionista no encontrado.")?;
            person.first_name = first.into();
            person.last_name = last.into();
            person.phone = (!phone.is_empty()).then(|| phone.into());
            Ok(id)
        } else {
            let id = self
                .pensioners
                .iter()
                .map(|p| p.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("No quedan identificadores disponibles.")?;
            self.pensioners.push(Pensioner {
                id,
                first_name: first.into(),
                last_name: last.into(),
                phone: (!phone.is_empty()).then(|| phone.into()),
                active: true,
                fingerprint_registered: false,
                biometric_id: None,
            });
            Ok(id)
        }
    }
    pub fn consumed_total(&self, id: u32) -> i64 {
        self.consumptions
            .iter()
            .filter(|c| c.pensioner_id == id)
            .map(|c| c.cents)
            .sum()
    }

    pub fn paid_total(&self, id: u32) -> i64 {
        self.payments
            .iter()
            .filter(|p| p.pensioner_id == id)
            .map(|p| p.cents)
            .sum()
    }

    pub fn net_balance(&self, id: u32) -> i64 {
        self.consumed_total(id) - self.paid_total(id)
    }

    pub fn balance(&self, id: u32) -> i64 {
        self.net_balance(id).max(0)
    }

    #[allow(dead_code)]
    pub fn credit_balance(&self, id: u32) -> i64 {
        (-self.net_balance(id)).max(0)
    }

    #[allow(dead_code)]
    pub fn tickets_for_date(&self, date: NaiveDate) -> (u32, i64) {
        self.payments
            .iter()
            .filter(|p| p.date == date)
            .filter_map(|p| match p.kind {
                PaymentKind::TicketCredit { count, .. } => Some((count, p.cents)),
                _ => None,
            })
            .fold((0, 0), |(c, a), (cnt, cents)| (c + cnt, a + cents))
    }

    #[allow(dead_code)]
    pub fn tickets_for_month(&self, year: i32, month: u32) -> (u32, i64) {
        use chrono::Datelike as _;
        self.payments
            .iter()
            .filter(|p| p.date.year() == year && p.date.month() == month)
            .filter_map(|p| match p.kind {
                PaymentKind::TicketCredit { count, .. } => Some((count, p.cents)),
                _ => None,
            })
            .fold((0, 0), |(c, a), (cnt, cents)| (c + cnt, a + cents))
    }

    pub fn tickets_for_pensioner(&self, pensioner_id: u32) -> (u32, i64) {
        self.payments
            .iter()
            .filter(|p| p.pensioner_id == pensioner_id)
            .filter_map(|p| match p.kind {
                PaymentKind::TicketCredit { count, .. } => Some((count, p.cents)),
                _ => None,
            })
            .fold((0, 0), |(c, a), (cnt, cents)| (c + cnt, a + cents))
    }

    pub fn total_tickets(&self) -> (u32, i64) {
        self.payments
            .iter()
            .filter_map(|p| match p.kind {
                PaymentKind::TicketCredit { count, .. } => Some((count, p.cents)),
                _ => None,
            })
            .fold((0, 0), |(c, a), (cnt, cents)| (c + cnt, a + cents))
    }

    pub fn monthly_summary(&self, id: u32) -> Vec<MonthlyPensionerSummary> {
        use chrono::Datelike as _;
        let mut by_month: std::collections::BTreeMap<(i32, u32), MonthlyPensionerSummary> =
            std::collections::BTreeMap::new();

        for c in self.consumptions.iter().filter(|c| c.pensioner_id == id) {
            let key = (c.date.year(), c.date.month());
            let entry = by_month
                .entry(key)
                .or_insert_with(|| MonthlyPensionerSummary {
                    month_label: format!("{:02}/{}", c.date.month(), c.date.year()),
                    consumed_cents: 0,
                    cash_paid_cents: 0,
                    tickets_cents: 0,
                    net_balance_cents: 0,
                });
            entry.consumed_cents += c.cents;
        }

        for p in self.payments.iter().filter(|p| p.pensioner_id == id) {
            let key = (p.date.year(), p.date.month());
            let entry = by_month
                .entry(key)
                .or_insert_with(|| MonthlyPensionerSummary {
                    month_label: format!("{:02}/{}", p.date.month(), p.date.year()),
                    consumed_cents: 0,
                    cash_paid_cents: 0,
                    tickets_cents: 0,
                    net_balance_cents: 0,
                });
            match &p.kind {
                PaymentKind::CashPayment => entry.cash_paid_cents += p.cents,
                PaymentKind::TicketCredit { .. } => entry.tickets_cents += p.cents,
            }
        }

        by_month
            .into_values()
            .rev()
            .map(|mut m| {
                m.net_balance_cents =
                    m.consumed_cents - (m.cash_paid_cents + m.tickets_cents);
                m
            })
            .collect()
    }

    fn next_movement_id(&mut self) -> String {
        let id = format!("MOV-{:04}", self.cash_register.next_movement_seq);
        self.cash_register.next_movement_seq += 1;
        id
    }

    pub fn open_cash_register(
        &mut self,
        date: NaiveDate,
        time: &str,
        opening_cents: i64,
        request_token: u64,
        processed_tokens: &mut HashSet<u64>,
    ) -> Result<String, String> {
        if processed_tokens.contains(&request_token) {
            return Err("La apertura ya fue registrada (se evitó duplicado).".into());
        }
        if self.cash_register.is_open {
            return Err("La caja ya se encuentra abierta.".into());
        }
        if opening_cents < 0 {
            return Err("El monto inicial de caja no puede ser negativo.".into());
        }
        processed_tokens.insert(request_token);
        let mov_id = self.next_movement_id();
        self.cash_register.is_open = true;
        self.cash_register.opening_cents = opening_cents;
        self.cash_register.movements.push(CashMovement {
            id: mov_id.clone(),
            date,
            time: time.to_string(),
            kind: CashMovementKind::Opening,
            pensioner_id: None,
            description: "Apertura de caja simulada".into(),
            cents: opening_cents,
            operator_name: "Sistema".into(),
            ticket_count: None,
            ticket_unit_cents: None,
            consumption_cents: None,
            resulting_balance_cents: None,
        });
        Ok(mov_id)
    }

    pub fn cash_inflow_cents(&self) -> i64 {
        self.cash_register
            .movements
            .iter()
            .filter(|m| matches!(m.kind, CashMovementKind::Payment))
            .map(|m| m.cents)
            .sum()
    }

    pub fn close_cash_register(
        &mut self,
        date: NaiveDate,
        time: &str,
        request_token: u64,
        processed_tokens: &mut HashSet<u64>,
    ) -> Result<(String, i64), String> {
        if processed_tokens.contains(&request_token) {
            return Err("El cierre ya fue registrado (se evitó duplicado).".into());
        }
        if !self.cash_register.is_open {
            return Err("La caja ya está cerrada.".into());
        }
        processed_tokens.insert(request_token);
        let expected_cents = self.cash_register.opening_cents + self.cash_inflow_cents();
        let mov_id = self.next_movement_id();
        self.cash_register.is_open = false;
        self.cash_register.movements.push(CashMovement {
            id: mov_id.clone(),
            date,
            time: time.to_string(),
            kind: CashMovementKind::Closing,
            pensioner_id: None,
            description: "Cierre y arqueo simulado de caja".into(),
            cents: expected_cents,
            operator_name: "Sistema".into(),
            ticket_count: None,
            ticket_unit_cents: None,
            consumption_cents: None,
            resulting_balance_cents: None,
        });
        Ok((mov_id, expected_cents))
    }

    pub fn record_ticket_credit(
        &mut self,
        date: NaiveDate,
        time: &str,
        pensioner_id: u32,
        ticket_count: u32,
        request_token: u64,
        processed_tokens: &mut HashSet<u64>,
    ) -> Result<String, String> {
        if processed_tokens.contains(&request_token) {
            return Err("Este abono por tickets ya fue contabilizado (se evitó duplicado).".into());
        }
        if !self.cash_register.is_open {
            return Err("La caja está cerrada. Ábrela antes de registrar movimientos.".into());
        }
        if !self.pensioners.iter().any(|p| p.id == pensioner_id) {
            return Err("Pensionista no encontrado.".into());
        }
        if ticket_count == 0 {
            return Err("Ingresa al menos 1 ticket físico.".into());
        }
        let unit = self.cash_register.ticket_value_cents;
        if unit <= 0 {
            return Err("El valor configurado del ticket debe ser mayor a S/ 0.00.".into());
        }
        let total = unit
            .checked_mul(ticket_count as i64)
            .ok_or_else(|| "El monto de tickets excede el límite permitido.".to_string())?;
        processed_tokens.insert(request_token);
        let mov_id = self.next_movement_id();
        self.push_ticket_credit_records(
            date,
            time,
            pensioner_id,
            ticket_count,
            unit,
            total,
            None,
            &mov_id,
        );
        Ok(mov_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn push_ticket_credit_records(
        &mut self,
        date: NaiveDate,
        time: &str,
        pensioner_id: u32,
        ticket_count: u32,
        unit_cents: i64,
        total_cents: i64,
        consumption_cents: Option<i64>,
        mov_id: &str,
    ) {
        self.payments.push(Payment {
            date,
            time: time.to_string(),
            pensioner_id,
            cents: total_cents,
            movement_id: mov_id.to_string(),
            kind: PaymentKind::TicketCredit {
                count: ticket_count,
                unit_cents,
            },
        });
        let person_name = self
            .pensioners
            .iter()
            .find(|p| p.id == pensioner_id)
            .map(|p| p.name())
            .unwrap_or_else(|| "Pensionista".to_string());
        let resulting_balance = self.net_balance(pensioner_id);
        self.cash_register.movements.push(CashMovement {
            id: mov_id.to_string(),
            date,
            time: time.to_string(),
            kind: CashMovementKind::TicketCredit,
            pensioner_id: Some(pensioner_id),
            description: format!(
                "{ticket_count} ticket(s) × S/ {}.{:02} · {person_name}",
                unit_cents / 100,
                unit_cents % 100
            ),
            cents: total_cents,
            operator_name: "Sistema".into(),
            ticket_count: Some(ticket_count),
            ticket_unit_cents: Some(unit_cents),
            consumption_cents,
            resulting_balance_cents: Some(resulting_balance),
        });
    }

    pub fn set_movement_operator(&mut self, movement_id: &str, operator: &str) {
        let clean = operator.trim();
        if clean.is_empty() {
            return;
        }
        if let Some(mov) = self
            .cash_register
            .movements
            .iter_mut()
            .find(|m| m.id == movement_id)
        {
            mov.operator_name = clean.to_string();
        }
    }
    pub fn set_active(&mut self, id: u32, expected: bool) -> Result<(), &'static str> {
        let person = self
            .pensioners
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Pensionista no encontrado.")?;
        if person.active != expected {
            return Err("El estado cambió; vuelve a intentarlo.");
        }
        person.active = !person.active;
        Ok(())
    }
    #[cfg(test)]
    pub fn register_fingerprint(&mut self, id: u32) -> Result<(), &'static str> {
        let default_bio = format!("BIO-{id:04}");
        self.register_fingerprint_with_id(id, &default_bio)
            .map(|_| ())
            .map_err(|_| "El pensionista no existe, está inactivo o el código biométrico está duplicado.")
    }

    pub fn register_fingerprint_with_id(
        &mut self,
        id: u32,
        biometric_id: &str,
    ) -> Result<String, String> {
        let clean_bio = biometric_id.trim();
        if clean_bio.is_empty() {
            return Err("Ingresa un identificador biométrico válido.".into());
        }
        let person = self.pensioners.iter().find(|p| p.id == id)
            .ok_or_else(|| "Pensionista no encontrado.".to_string())?;
        if !person.active { return Err("El pensionista está inactivo.".into()); }
        if person.fingerprint_registered || person.biometric_id.is_some() {
            return Err("Este pensionista ya tiene una huella registrada.".into());
        }
        if let Some(other) = self.pensioners.iter().find(|p| {
            p.id != id
                && p.biometric_id
                    .as_deref()
                    .is_some_and(|existing| existing.eq_ignore_ascii_case(clean_bio))
        }) {
            return Err(format!(
                "El identificador biométrico '{clean_bio}' ya está asociado a {} (#{}).",
                other.name(),
                other.id
            ));
        }
        let person = self
            .pensioners
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "Pensionista no encontrado.".to_string())?;
        person.fingerprint_registered = true;
        person.biometric_id = Some(clean_bio.to_string());
        Ok(clean_bio.to_string())
    }

    pub fn biometric_candidates(&self) -> Vec<crate::biometric_service::BiometricCandidate> {
        self.pensioners
            .iter()
            .filter_map(|p| {
                if p.fingerprint_registered {
                    p.biometric_id.as_ref().map(|bio| {
                        crate::biometric_service::BiometricCandidate {
                            pensioner_id: p.id,
                            biometric_id: bio.clone(),
                            active: p.active,
                        }
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Registra consumos y opcionalmente tickets físicos entregados en la misma atención
    /// de forma atómica e idempotente.
    pub fn record_consumptions_with_tickets(
        &mut self,
        date: NaiveDate,
        time: &str,
        pensioner_id: u32,
        items: &[DraftConsumptionItem],
        picked_up_by: Option<&str>,
        ticket_count: u32,
        request_token: u64,
        processed_tokens: &mut HashSet<u64>,
    ) -> Result<(usize, Option<String>, i64), String> {
        if processed_tokens.contains(&request_token) {
            return Err("Esta atención ya fue registrada (se evitó un duplicado).".into());
        }
        if items.is_empty() {
            return Err("Agrega al menos una ración antes de confirmar.".into());
        }
        let person = self
            .pensioners
            .iter()
            .find(|p| p.id == pensioner_id)
            .ok_or_else(|| "Pensionista no encontrado.".to_string())?;
        if !person.active {
            return Err("No se puede registrar consumo para un pensionista inactivo.".into());
        }
        let clean_pickup = match picked_up_by {
            Some(name) => {
                let trimmed = name.trim();
                if trimmed.chars().count() < 2 {
                    return Err(
                        "Indica el nombre completo de la persona que recoge el consumo.".into(),
                    );
                }
                Some(trimmed.to_string())
            }
            None => None,
        };

        let mut consumption_total_cents: i64 = 0;
        for item in items {
            if item.portions == 0 || item.unit_price_cents <= 0 {
                return Err("Todas las raciones deben tener cantidad y precio válidos.".into());
            }
            consumption_total_cents = consumption_total_cents
                .checked_add(item.subtotal_cents())
                .ok_or_else(|| "El monto del consumo excede el límite permitido.".to_string())?;
        }

        let ticket_plan = if ticket_count > 0 {
            if !self.cash_register.is_open {
                return Err(
                    "La caja está cerrada. Ábrela antes de recibir tickets físicos, o registra el consumo como deuda.".into(),
                );
            }
            let unit = self.cash_register.ticket_value_cents;
            if unit <= 0 {
                return Err("El valor configurado del ticket debe ser mayor a S/ 0.00.".into());
            }
            let tickets_total = unit
                .checked_mul(ticket_count as i64)
                .ok_or_else(|| "El monto de tickets excede el límite permitido.".to_string())?;
            Some((unit, tickets_total))
        } else {
            None
        };

        processed_tokens.insert(request_token);
        let count = items.len();
        for item in items {
            self.consumptions.push(Consumption {
                date,
                time: time.to_string(),
                pensioner_id,
                meal: item.meal,
                portions: item.portions,
                cents: item.subtotal_cents(),
                menu_kind: item.menu_kind,
                detail: (!item.description.trim().is_empty())
                    .then(|| item.description.trim().to_string()),
                picked_up_by: clean_pickup.clone(),
            });
        }

        let mov_id = if let Some((unit, total)) = ticket_plan {
            let mov_id = self.next_movement_id();
            self.push_ticket_credit_records(
                date,
                time,
                pensioner_id,
                ticket_count,
                unit,
                total,
                Some(consumption_total_cents),
                &mov_id,
            );
            Some(mov_id)
        } else {
            None
        };

        let final_net_balance = self.net_balance(pensioner_id);
        Ok((count, mov_id, final_net_balance))
    }

    #[allow(dead_code)]
    pub fn record_consumptions(
        &mut self,
        date: NaiveDate,
        time: &str,
        pensioner_id: u32,
        items: &[DraftConsumptionItem],
        picked_up_by: Option<&str>,
        request_token: u64,
        processed_tokens: &mut HashSet<u64>,
    ) -> Result<usize, String> {
        self.record_consumptions_with_tickets(
            date,
            time,
            pensioner_id,
            items,
            picked_up_by,
            0,
            request_token,
            processed_tokens,
        )
        .map(|(count, _, _)| count)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        if self.pensioners.iter().any(|p| !ids.insert(p.id)) {
            return Err("Hay identificadores de pensionista duplicados".into());
        }
        let mut bio_ids = std::collections::HashSet::new();
        for p in &self.pensioners {
            if let Some(bio) = &p.biometric_id {
                if !bio_ids.insert(bio.to_ascii_lowercase()) {
                    return Err("Hay identificadores biométricos duplicados".into());
                }
            }
        }
        if self
            .consumptions
            .iter()
            .any(|c| !ids.contains(&c.pensioner_id) || c.cents < 0 || c.portions == 0)
            || self
                .payments
                .iter()
                .any(|p| !ids.contains(&p.pensioner_id) || p.cents < 0)
        {
            return Err("Los datos de ejemplo contienen operaciones inválidas".into());
        }
        Ok(())
    }
    pub fn demo(today: NaiveDate) -> Self {
        let yesterday = today - Duration::days(1);
        Self {
            pensioners: vec![
                Pensioner {
                    id: 1,
                    first_name: "Ana".into(),
                    last_name: "Torres".into(),
                    phone: None,
                    active: true,
                    fingerprint_registered: true,
                    biometric_id: Some("BIO-0001".into()),
                },
                Pensioner {
                    id: 2,
                    first_name: "Luis".into(),
                    last_name: "Rojas".into(),
                    phone: None,
                    active: true,
                    fingerprint_registered: false,
                    biometric_id: None,
                },
                Pensioner {
                    id: 3,
                    first_name: "María".into(),
                    last_name: "Vega".into(),
                    phone: None,
                    active: true,
                    fingerprint_registered: false,
                    biometric_id: None,
                },
                Pensioner {
                    id: 4,
                    first_name: "Jorge".into(),
                    last_name: "Ruiz".into(),
                    phone: None,
                    active: false,
                    fingerprint_registered: false,
                    biometric_id: None,
                },
            ],
            consumptions: vec![
                Consumption {
                    date: today,
                    time: "08:10".into(),
                    pensioner_id: 1,
                    meal: Meal::Breakfast,
                    portions: 1,
                    cents: 500,
                    menu_kind: MenuKind::Economic,
                    detail: None,
                    picked_up_by: None,
                },
                Consumption {
                    date: today,
                    time: "08:25".into(),
                    pensioner_id: 2,
                    meal: Meal::Breakfast,
                    portions: 2,
                    cents: 1000,
                    menu_kind: MenuKind::Economic,
                    detail: None,
                    picked_up_by: None,
                },
                Consumption {
                    date: today,
                    time: "12:15".into(),
                    pensioner_id: 1,
                    meal: Meal::Lunch,
                    portions: 1,
                    cents: 900,
                    menu_kind: MenuKind::Economic,
                    detail: None,
                    picked_up_by: None,
                },
                Consumption {
                    date: today,
                    time: "12:40".into(),
                    pensioner_id: 3,
                    meal: Meal::Lunch,
                    portions: 2,
                    cents: 1800,
                    menu_kind: MenuKind::Economic,
                    detail: None,
                    picked_up_by: None,
                },
                Consumption {
                    date: yesterday,
                    time: "12:10".into(),
                    pensioner_id: 2,
                    meal: Meal::Lunch,
                    portions: 1,
                    cents: 900,
                    menu_kind: MenuKind::Economic,
                    detail: None,
                    picked_up_by: None,
                },
            ],
            payments: vec![
                Payment {
                    date: today,
                    time: "13:05".into(),
                    pensioner_id: 1,
                    cents: 1000,
                    movement_id: "MOV-0002".into(),
                    kind: PaymentKind::CashPayment,
                },
                Payment {
                    date: yesterday,
                    time: "15:00".into(),
                    pensioner_id: 2,
                    cents: 500,
                    movement_id: "MOV-0003".into(),
                    kind: PaymentKind::CashPayment,
                },
            ],
            cash_register: CashRegisterState {
                is_open: true,
                opening_cents: 10000,
                ticket_value_cents: 700,
                next_movement_seq: 4,
                movements: vec![
                    CashMovement {
                        id: "MOV-0001".into(),
                        date: yesterday,
                        time: "07:30".into(),
                        kind: CashMovementKind::Opening,
                        pensioner_id: None,
                        description: "Apertura inicial de caja simulada".into(),
                        cents: 10000,
                        operator_name: "Cajero de prueba".into(),
                        ticket_count: None,
                        ticket_unit_cents: None,
                        consumption_cents: None,
                        resulting_balance_cents: None,
                    },
                    CashMovement {
                        id: "MOV-0002".into(),
                        date: today,
                        time: "13:05".into(),
                        kind: CashMovementKind::Payment,
                        pensioner_id: Some(1),
                        description: "Pago parcial · Juan Pérez".into(),
                        cents: 1000,
                        operator_name: "Cajero de prueba".into(),
                        ticket_count: None,
                        ticket_unit_cents: None,
                        consumption_cents: None,
                        resulting_balance_cents: None,
                    },
                    CashMovement {
                        id: "MOV-0003".into(),
                        date: yesterday,
                        time: "15:00".into(),
                        kind: CashMovementKind::Payment,
                        pensioner_id: Some(2),
                        description: "Pago parcial · María Gómez".into(),
                        cents: 500,
                        operator_name: "Administradora de prueba".into(),
                        ticket_count: None,
                        ticket_unit_cents: None,
                        consumption_cents: None,
                        resulting_balance_cents: None,
                    },
                ],
            },
        }
    }
    /// Los saldos se calculan por persona hasta la fecha elegida, sin compensar
    /// los saldos a favor de una persona con la deuda de otra.
    pub fn summary(&self, date: NaiveDate) -> DashboardSummary {
        let mut result = DashboardSummary {
            registered: self.pensioners.len(),
            active: self.pensioners.iter().filter(|p| p.active).count(),
            ..Default::default()
        };
        let names: HashMap<_, _> = self.pensioners.iter().map(|p| (p.id, p.name())).collect();
        let mut balances: HashMap<u32, i64> = HashMap::new();
        for c in &self.consumptions {
            if c.date <= date {
                *balances.entry(c.pensioner_id).or_default() += c.cents;
            }
            if c.date == date {
                match c.meal {
                    Meal::Breakfast => {
                        result.breakfasts += 1;
                        result.breakfast_portions += c.portions;
                    }
                    Meal::Lunch => {
                        result.lunches += 1;
                        result.lunch_portions += c.portions;
                    }
                }
                result.portions += c.portions;
                result.consumed_cents += c.cents;
                result.activity.push(Activity {
                    time: c.time.clone(),
                    person: names
                        .get(&c.pensioner_id)
                        .cloned()
                        .unwrap_or_else(|| "Pensionista desconocido".into()),
                    kind: c.meal.label(),
                    portions: Some(c.portions),
                    cents: c.cents,
                });
            }
        }
        for p in &self.payments {
            if p.date <= date {
                *balances.entry(p.pensioner_id).or_default() -= p.cents;
            }
            if p.date == date {
                result.paid_cents += p.cents;
                let kind_label = match &p.kind {
                    PaymentKind::CashPayment => {
                        result.cash_paid_cents += p.cents;
                        "Pago"
                    }
                    PaymentKind::TicketCredit { count, .. } => {
                        result.tickets_count += *count;
                        result.tickets_cents += p.cents;
                        "Tickets"
                    }
                };
                let portions = match &p.kind {
                    PaymentKind::TicketCredit { count, .. } => Some(*count),
                    _ => None,
                };
                result.activity.push(Activity {
                    time: p.time.to_string(),
                    person: names
                        .get(&p.pensioner_id)
                        .cloned()
                        .unwrap_or_else(|| "Pensionista desconocido".into()),
                    kind: kind_label,
                    portions,
                    cents: p.cents,
                });
            }
        }
        result.debt_cents = balances.values().map(|v| (*v).max(0)).sum();
        result.credit_balance_cents = balances.values().map(|v| (-*v).max(0)).sum();
        result.activity.sort_by(|a, b| b.time.cmp(&a.time));
        result
    }
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn totals_keep_transactions_portions_and_money_distinct() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let data = MockData::demo(today);
        let s = data.summary(today);
        assert_eq!(
            (s.registered, s.active, s.breakfasts, s.lunches, s.portions),
            (4, 3, 2, 2, 6)
        );
        assert_eq!(
            (s.consumed_cents, s.paid_cents, s.debt_cents),
            (4200, 1000, 3600)
        );
        assert_eq!(s.activity.first().unwrap().kind, "Pago");
        let yesterday = data.summary(today - Duration::days(1));
        assert_eq!(
            (yesterday.lunches, yesterday.portions, yesterday.debt_cents),
            (1, 1, 400)
        );
        assert!(data.summary(today + Duration::days(1)).activity.is_empty());
    }
    #[test]
    fn new_shared_records_are_reflected_without_cached_totals() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        data.payments.push(Payment {
            date,
            time: "14:00".into(),
            pensioner_id: 3,
            cents: 5000,
            movement_id: "MOV-0099".into(),
            kind: PaymentKind::CashPayment,
        });
        let s = data.summary(date);
        assert_eq!(s.paid_cents, 6000);
        assert_eq!(s.debt_cents, 1800);
    }
    #[test]
    fn invalid_records_are_rejected() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        assert!(data.validate().is_ok());
        data.consumptions[0].pensioner_id = 999;
        assert!(data.validate().is_err());
    }
    #[test]
    fn pensioner_edits_validate_and_keep_identity_and_history() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        assert!(data.save_pensioner(None, " ", "García", "").is_err());
        assert!(data.save_pensioner(None, "Rosa", "García", "abc").is_err());
        let id = data
            .save_pensioner(None, " Rosa ", " García ", "987654321")
            .unwrap();
        assert_eq!(id, 5);
        data.save_pensioner(Some(1), "Anita", "Torres", "").unwrap();
        assert_eq!(data.pensioners[0].id, 1);
        assert_eq!(data.balance(1), 400);
        assert_eq!(
            data.summary(date)
                .activity
                .iter()
                .find(|a| a.person == "Anita Torres")
                .unwrap()
                .person,
            "Anita Torres"
        );
        assert!(data.save_pensioner(Some(99), "A", "B", "").is_err());
    }
    #[test]
    fn status_changes_keep_biometrics_and_update_dashboard() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        assert_eq!(data.pensioners[1].biometric_status(), "Pendiente");
        data.register_fingerprint(2).unwrap();
        assert_eq!(data.pensioners[1].biometric_status(), "Registrado");
        data.set_active(2, true).unwrap();
        assert_eq!(data.pensioners[1].biometric_status(), "Inactivo");
        assert_eq!(data.summary(date).active, 2);
        assert!(data.register_fingerprint(2).is_err());
        assert!(data.set_active(2, true).is_err());
        data.set_active(2, false).unwrap();
        assert_eq!(data.pensioners[1].biometric_status(), "Registrado");
        assert_eq!(data.summary(date).active, 3);
    }
    #[test]
    fn duplicate_biometric_id_cannot_be_assigned_to_two_pensioners() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        assert!(data.register_fingerprint_with_id(2, "BIO-0001").is_err());
        assert!(data.register_fingerprint_with_id(2, "BIO-0002").is_ok());
        assert_eq!(data.pensioners[1].biometric_id.as_deref(), Some("BIO-0002"));
        assert!(data.register_fingerprint_with_id(2, "BIO-0003").is_err());
        assert!(data.register_fingerprint_with_id(2, "BIO-0002").is_err());
        assert!(data.register_fingerprint_with_id(3, "bio-0002").is_err());
        assert_eq!(data.pensioners[1].biometric_id.as_deref(), Some("BIO-0002"));
        assert!(!data.pensioners[2].fingerprint_registered);
    }
    #[test]
    fn consumptions_support_multiple_items_third_party_and_prevent_duplicates() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        let pricing = ConsumptionPricing::default();
        assert_eq!(pricing.economic_for(Meal::Breakfast), 700);
        assert_eq!(pricing.economic_for(Meal::Lunch), 700);
        assert_eq!(parse_price_cents("7").unwrap(), 700);
        assert_eq!(parse_price_cents("12.50").unwrap(), 1250);
        assert!(parse_price_cents("0").is_err());

        let items = vec![
            DraftConsumptionItem {
                id: 1,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Economic,
                description: "Económico".into(),
                portions: 2,
                unit_price_cents: 700,
            },
            DraftConsumptionItem {
                id: 2,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Special,
                description: "Seco especial".into(),
                portions: 1,
                unit_price_cents: 1400,
            },
        ];

        let mut processed = HashSet::new();
        assert!(
            data.record_consumptions(date, "13:00", 1, &items, Some(""), 10, &mut processed)
                .is_err()
        );
        assert!(
            data.record_consumptions(date, "13:00", 4, &items, None, 10, &mut processed)
                .is_err()
        );
        let time = String::from("13:37");
        assert_eq!(
            data.record_consumptions(
                date,
                &time,
                1,
                &items,
                Some("Pedro Torres"),
                10,
                &mut processed
            )
            .unwrap(),
            2
        );
        drop(time);
        assert_eq!(data.consumptions.last().unwrap().time, "13:37");
        assert!(
            data.record_consumptions(
                date,
                "13:00",
                1,
                &items,
                Some("Pedro Torres"),
                10,
                &mut processed
            )
            .is_err()
        );
        assert_eq!(data.balance(1), 3200);
    }
    #[test]
    fn cash_register_tickets_and_idempotency() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        let mut data = MockData::demo(date);
        let mut tokens = HashSet::new();

        // Pensionista #3 tiene consumo de 1800 (S/ 18.00) y 0 pagos -> deuda 1800
        assert_eq!(data.balance(3), 1800);
        assert_eq!(data.cash_register.ticket_value_cents, 700);

        assert!(data.record_ticket_credit(date, "14:00", 3, 0, 1, &mut tokens).is_err());

        // 2 tickets de S/ 7.00 = S/ 14.00 (1400 céntimos), deuda queda en 400
        let mov_t = data
            .record_ticket_credit(date, "14:05", 3, 2, 2, &mut tokens)
            .unwrap();
        assert_eq!(mov_t, "MOV-0004");
        assert_eq!(data.balance(3), 400);

        // Evita contabilizar dos veces el mismo movimiento
        assert!(data.record_ticket_credit(date, "14:05", 3, 2, 2, &mut tokens).is_err());

        // Otro ticket cubre la deuda y genera saldo a favor.
        let mov_p = data
            .record_ticket_credit(date, "14:15", 3, 1, 4, &mut tokens)
            .unwrap();
        assert_eq!(mov_p, "MOV-0005");
        assert_eq!(data.balance(3), 0);
        assert_eq!(data.credit_balance(3), 300);

        // Resumen mensual coherente
        let monthly = data.monthly_summary(3);
        assert_eq!(monthly.len(), 1);
        assert_eq!(monthly[0].consumed_cents, 1800);
        assert_eq!(monthly[0].tickets_cents, 2100);
        assert_eq!(monthly[0].cash_paid_cents, 0);
        assert_eq!(monthly[0].net_balance_cents, -300);

        // Cierre de caja bloquea nuevos abonos hasta reabrir.
        let (_close_id, expected) = data.close_cash_register(date, "20:00", 5, &mut tokens).unwrap();
        assert_eq!(expected, 10000 + 1000 + 500);
        assert!(data.record_ticket_credit(date, "20:05", 1, 1, 6, &mut tokens).is_err());
        data.open_cash_register(date, "21:00", 5000, 7, &mut tokens)
            .unwrap();
        assert!(data.cash_register.is_open);
    }

    #[test]
    fn consumptions_with_physical_tickets_combine_debt_cash_and_prevent_duplicates() {
        let date = today();
        let mut data = MockData::demo(date);
        let mut tokens = HashSet::new();
        let pid = data.save_pensioner(None, "Rosa", "Mendoza", "988777666").unwrap();
        assert_eq!(data.balance(pid), 0);
        assert_eq!(data.cash_register.ticket_value_cents, 700);

        // Consumo mixto de S/ 19.00 (1 económico S/ 7.00 + 1 especial S/ 12.00) con 1 ticket de S/ 7.00 -> deuda S/ 12.00
        let items = vec![
            DraftConsumptionItem {
                id: 1,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Economic,
                portions: 1,
                unit_price_cents: 700,
                description: String::new(),
            },
            DraftConsumptionItem {
                id: 2,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Special,
                portions: 1,
                unit_price_cents: 1200,
                description: "Seco".into(),
            },
        ];

        let (count, mov_opt, net_debt) = data
            .record_consumptions_with_tickets(date, "12:30", pid, &items, None, 1, 100, &mut tokens)
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(mov_opt.as_deref(), Some("MOV-0004"));
        assert_eq!(net_debt, 1200);
        assert_eq!(data.balance(pid), 1200);

        // Verifica que Caja y resumen mensual reflejen el ticket inmediatamente
        assert!(data.cash_register.movements.iter().any(|m| {
            m.id == "MOV-0004" && m.kind == CashMovementKind::TicketCredit && m.cents == 700
        }));
        let monthly = data.monthly_summary(pid);
        assert_eq!(monthly[0].consumed_cents, 1900);
        assert_eq!(monthly[0].tickets_cents, 700);
        assert_eq!(monthly[0].net_balance_cents, 1200);

        // Idempotencia: repetir el mismo token no duplica consumo ni ticket
        assert!(data
            .record_consumptions_with_tickets(date, "12:30", pid, &items, None, 1, 100, &mut tokens)
            .is_err());
        assert_eq!(data.balance(pid), 1200);

        // Si la caja está cerrada y se entregan tickets, falla atómicamente sin registrar consumo ni ticket
        data.cash_register.is_open = false;
        let prev_consumptions = data.consumptions.len();
        let prev_movements = data.cash_register.movements.len();
        assert!(data
            .record_consumptions_with_tickets(date, "12:40", pid, &items, None, 1, 101, &mut tokens)
            .is_err());
        assert_eq!(data.consumptions.len(), prev_consumptions);
        assert_eq!(data.cash_register.movements.len(), prev_movements);
        assert!(!tokens.contains(&101));
        data.cash_register.is_open = true;

        // Registro posterior de tickets desde Caja sobre la deuda restante
        let mov_later = data
            .record_ticket_credit(date, "15:00", pid, 1, 102, &mut tokens)
            .unwrap();
        assert_eq!(mov_later, "MOV-0005");
        assert_eq!(data.balance(pid), 500);
    }

    #[test]
    fn consumptions_with_surplus_tickets_create_credit_balance_and_auto_apply() {
        let date = today();
        let mut data = MockData::demo(date);
        let mut tokens = HashSet::new();
        let pid = data.save_pensioner(None, "Carlos", "Quispe", "911222333").unwrap();

        // Consumo de S/ 23.00 (1 económico S/ 7.00 + 1 especial S/ 16.00), entrega 6 tickets de S/ 7.00 = S/ 42.00
        let items_23 = vec![
            DraftConsumptionItem {
                id: 1,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Economic,
                portions: 1,
                unit_price_cents: 700,
                description: String::new(),
            },
            DraftConsumptionItem {
                id: 2,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Special,
                portions: 1,
                unit_price_cents: 1600,
                description: "Parrilla".into(),
            },
        ];

        let (_count, mov_opt, final_net) = data
            .record_consumptions_with_tickets(date, "12:15", pid, &items_23, None, 6, 200, &mut tokens)
            .unwrap();
        assert_eq!(mov_opt.as_deref(), Some("MOV-0004"));
        assert_eq!(final_net, -1900);
        assert_eq!(data.balance(pid), 0);
        assert_eq!(data.credit_balance(pid), 1900);

        // Futuro consumo de S/ 12.00 sin entregar tickets -> se descuenta del saldo a favor, queda S/ 7.00 a favor
        let item_12 = vec![DraftConsumptionItem {
            id: 3,
            meal: Meal::Lunch,
            menu_kind: MenuKind::Special,
            portions: 1,
            unit_price_cents: 1200,
            description: "Lomo".into(),
        }];
        let (_count, mov_none, final_net2) = data
            .record_consumptions_with_tickets(date, "13:00", pid, &item_12, None, 0, 201, &mut tokens)
            .unwrap();
        assert!(mov_none.is_none());
        assert_eq!(final_net2, -700);
        assert_eq!(data.balance(pid), 0);
        assert_eq!(data.credit_balance(pid), 700);

        // Otro consumo de S/ 10.00 -> consume los S/ 7.00 a favor y queda S/ 3.00 de deuda
        let item_10 = vec![DraftConsumptionItem {
            id: 4,
            meal: Meal::Lunch,
            menu_kind: MenuKind::Special,
            portions: 1,
            unit_price_cents: 1000,
            description: "Milanesa".into(),
        }];
        let (_count, _mov_none2, final_net3) = data
            .record_consumptions_with_tickets(date, "14:00", pid, &item_10, None, 0, 202, &mut tokens)
            .unwrap();
        assert_eq!(final_net3, 300);
        assert_eq!(data.balance(pid), 300);
        assert_eq!(data.credit_balance(pid), 0);

        // Cambiar el precio unitario del ticket después NO altera el valor histórico ni los totales previos
        data.cash_register.ticket_value_cents = 900;
        assert_eq!(data.tickets_for_pensioner(pid), (6, 4200));
        assert_eq!(data.tickets_for_date(date), (6, 4200));
        use chrono::Datelike as _;
        assert_eq!(data.tickets_for_month(date.year(), date.month()), (6, 4200));

        // El abono posterior desde Caja con tickets también permite excedente y genera saldo a favor
        let mov_caja = data
            .record_ticket_credit(date, "15:10", pid, 1, 203, &mut tokens)
            .unwrap();
        assert_eq!(mov_caja, "MOV-0005");
        assert_eq!(data.balance(pid), 0);
        assert_eq!(data.credit_balance(pid), 600);
        assert_eq!(data.tickets_for_pensioner(pid), (7, 5100));

        // Dashboard separa efectivo ingresado de tickets recibidos
        let dash = data.summary(date);
        assert_eq!(dash.tickets_count, 7);
        assert_eq!(dash.tickets_cents, 5100);
        assert_eq!(dash.cash_paid_cents, 1000);
    }
}


