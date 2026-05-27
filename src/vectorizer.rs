use std::collections::HashMap;

use crate::models::TransactionRequest;
use crate::resources::Normalization;

pub fn vectorize(
    tx: &TransactionRequest,
    normalization: &Normalization,
    mcc_risk: &HashMap<String, f64>,
) -> [f64; 14] {
    let mut vec = [0.0f64; 14];

    let tx_requested_at = parse_timestamp(&tx.transaction.requested_at);

    vec[0] = (tx.transaction.amount / normalization.max_amount).clamp(0.0, 1.0);
    vec[1] = (tx.transaction.installments as f64 / normalization.max_installments).clamp(0.0, 1.0);
    vec[2] = ((tx.transaction.amount / tx.customer.avg_amount) / normalization.amount_vs_avg_ratio)
        .clamp(0.0, 1.0);
    vec[3] = (tx_requested_at.hour as f64 / 23.0).clamp(0.0, 1.0);
    vec[4] = (tx_requested_at.weekday as f64 / 6.0).clamp(0.0, 1.0);

    if let Some(last_tx) = &tx.last_transaction {
        let last_tx_time = parse_timestamp(&last_tx.timestamp);
        let minutes_elapsed = tx_requested_at.minutes - last_tx_time.minutes;
        vec[5] = (minutes_elapsed as f64 / normalization.max_minutes).clamp(0.0, 1.0);
        vec[6] = (last_tx.km_from_current / normalization.max_km).clamp(0.0, 1.0);
    } else {
        vec[5] = -1.0;
        vec[6] = -1.0;
    }

    vec[7] = (tx.terminal.km_from_home / normalization.max_km).clamp(0.0, 1.0);
    vec[8] = (tx.customer.tx_count_24h as f64 / normalization.max_tx_count_24h).clamp(0.0, 1.0);
    vec[9] = if tx.terminal.is_online { 1.0 } else { 0.0 };
    vec[10] = if tx.terminal.card_present { 1.0 } else { 0.0 };

    let know_merchant = tx.customer.known_merchants.contains(&tx.merchant.id);
    vec[11] = if know_merchant { 0.0 } else { 1.0 };
    vec[12] = *mcc_risk.get(&tx.merchant.mcc).unwrap_or(&0.5);
    vec[13] = (tx.merchant.avg_amount / normalization.max_merchant_avg_amount).clamp(0.0, 1.0);

    vec
}

struct Timestamp {
    minutes: i64,
    hour: u32,
    weekday: u32,
}

fn parse_timestamp(value: &str) -> Timestamp {
    let bytes = value.as_bytes();
    if bytes.len() >= 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
    {
        let year = parse_u32(&bytes[0..4]);
        let month = parse_u32(&bytes[5..7]);
        let day = parse_u32(&bytes[8..10]);
        let hour = parse_u32(&bytes[11..13]);
        let minute = parse_u32(&bytes[14..16]);

        if let (Some(year), Some(month), Some(day), Some(hour), Some(minute)) =
            (year, month, day, hour, minute)
        {
            let days = days_from_civil(year as i32, month as i32, day as i32);
            return Timestamp {
                minutes: days * 1440 + hour as i64 * 60 + minute as i64,
                hour,
                weekday: ((days + 3).rem_euclid(7)) as u32,
            };
        }
    }

    Timestamp {
        minutes: 0,
        hour: 0,
        weekday: 3,
    }
}

fn parse_u32(bytes: &[u8]) -> Option<u32> {
    let mut value = 0u32;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Some(value)
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era * 146097 + doe - 719468)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        CustomerData, LastTransaction, MerchantData, TerminalData, TransactionData,
    };

    fn norm() -> Normalization {
        Normalization {
            max_amount: 10000.0,
            max_installments: 12.0,
            amount_vs_avg_ratio: 10.0,
            max_minutes: 1440.0,
            max_km: 1000.0,
            max_tx_count_24h: 20.0,
            max_merchant_avg_amount: 10000.0,
        }
    }

    fn mcc() -> HashMap<String, f64> {
        HashMap::from([
            ("5411".into(), 0.15),
            ("5812".into(), 0.30),
            ("5912".into(), 0.20),
            ("5944".into(), 0.45),
            ("7801".into(), 0.80),
            ("7802".into(), 0.75),
            ("7995".into(), 0.85),
            ("4511".into(), 0.35),
            ("5311".into(), 0.25),
            ("5999".into(), 0.50),
        ])
    }

    #[test]
    fn test_legit_transaction() {
        let tx = TransactionRequest {
            id: "tx-1".into(),
            transaction: TransactionData {
                amount: 41.12,
                installments: 2,
                requested_at: "2026-03-11T18:45:53Z".into(),
            },
            customer: CustomerData {
                avg_amount: 82.24,
                tx_count_24h: 3,
                known_merchants: vec!["MERC-003".into(), "MERC-016".into()],
            },
            merchant: MerchantData {
                id: "MERC-016".into(),
                mcc: "5411".into(),
                avg_amount: 60.25,
            },
            terminal: TerminalData {
                is_online: false,
                card_present: true,
                km_from_home: 29.23,
            },
            last_transaction: None,
        };

        let v = vectorize(&tx, &norm(), &mcc());

        assert!((v[0] - 0.0041).abs() < 0.001);
        assert!((v[1] - 0.1667).abs() < 0.001);
        assert!((v[2] - 0.05).abs() < 0.001);
        assert!((v[3] - 0.783).abs() < 0.001);
        assert!((v[4] - 0.333).abs() < 0.001);
        assert_eq!(v[5], -1.0);
        assert_eq!(v[6], -1.0);
        assert!((v[7] - 0.029).abs() < 0.001);
        assert!((v[8] - 0.15).abs() < 0.001);
        assert_eq!(v[9], 0.0);
        assert_eq!(v[10], 1.0);
        assert_eq!(v[11], 0.0);
        assert!((v[12] - 0.15).abs() < 0.001);
        assert!((v[13] - 0.006).abs() < 0.001);
    }

    #[test]
    fn test_fraud_transaction() {
        let tx = TransactionRequest {
            id: "tx-2".into(),
            transaction: TransactionData {
                amount: 9505.97,
                installments: 10,
                requested_at: "2026-03-14T05:15:12Z".into(),
            },
            customer: CustomerData {
                avg_amount: 81.28,
                tx_count_24h: 20,
                known_merchants: vec!["MERC-008".into(), "MERC-007".into(), "MERC-005".into()],
            },
            merchant: MerchantData {
                id: "MERC-068".into(),
                mcc: "7802".into(),
                avg_amount: 54.86,
            },
            terminal: TerminalData {
                is_online: false,
                card_present: true,
                km_from_home: 952.27,
            },
            last_transaction: None,
        };

        let v = vectorize(&tx, &norm(), &mcc());

        assert!((v[0] - 0.951).abs() < 0.001);
        assert!((v[1] - 0.833).abs() < 0.001);
        assert!((v[2] - 1.0).abs() < 0.001);
        assert!((v[3] - 0.217).abs() < 0.001);
        assert!((v[4] - 0.833).abs() < 0.001);
        assert_eq!(v[5], -1.0);
        assert_eq!(v[6], -1.0);
        assert!((v[7] - 0.952).abs() < 0.001);
        assert!((v[8] - 1.0).abs() < 0.001);
        assert_eq!(v[9], 0.0);
        assert_eq!(v[10], 1.0);
        assert_eq!(v[11], 1.0);
        assert!((v[12] - 0.75).abs() < 0.001);
        assert!((v[13] - 0.005).abs() < 0.001);
    }

    #[test]
    fn test_with_last_transaction() {
        let tx = TransactionRequest {
            id: "tx-3".into(),
            transaction: TransactionData {
                amount: 100.0,
                installments: 1,
                requested_at: "2026-03-11T18:45:53Z".into(),
            },
            customer: CustomerData {
                avg_amount: 82.24,
                tx_count_24h: 3,
                known_merchants: vec!["MERC-003".into()],
            },
            merchant: MerchantData {
                id: "MERC-003".into(),
                mcc: "5411".into(),
                avg_amount: 60.25,
            },
            terminal: TerminalData {
                is_online: true,
                card_present: false,
                km_from_home: 50.0,
            },
            last_transaction: Some(LastTransaction {
                timestamp: "2026-03-11T17:45:53Z".into(),
                km_from_current: 100.0,
            }),
        };

        let v = vectorize(&tx, &norm(), &mcc());

        assert!((v[5] - 60.0 / 1440.0).abs() < 0.001);
        assert!((v[6] - 0.1).abs() < 0.001);
        assert_eq!(v[9], 1.0);
        assert_eq!(v[10], 0.0);
    }
}
