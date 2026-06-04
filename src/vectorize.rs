use crate::config::*;
use crate::models::TransactionPayload;
use chrono::{Datelike, Timelike};

fn clamp(n: f32) -> f32 {
    n.clamp(0.0, 1.0)
}

pub fn vectorize(payload: &TransactionPayload<'_>) -> [f32; 14] {
    let transaction = &payload.transaction;
    let customer = &payload.customer;
    let merchant = &payload.merchant;
    let terminal = &payload.terminal;

    let (minutes_since, km_from_last) = match &payload.last_transaction {
        None => (-1.0, -1.0),
        Some(last) => {
            let minutes =
                (transaction.requested_at - last.timestamp).num_minutes() as f32 / MAX_MINUTES;
            let km = last.km_from_current / MAX_KM;

            (clamp(minutes), clamp(km))
        }
    };

    [
        clamp(transaction.amount / MAX_AMOUNT),
        clamp(transaction.installments as f32 / MAX_INSTALLMENTS),
        clamp((transaction.amount / customer.avg_amount) / AMOUNT_VS_AVG_RATIO),
        transaction.requested_at.hour() as f32 / 23.0,
        transaction.requested_at.weekday().num_days_from_monday() as f32 / 6.0,
        minutes_since,
        km_from_last,
        clamp(terminal.km_from_home / MAX_KM),
        clamp(customer.tx_count_24h as f32 / MAX_TX_COUNT_24H),
        terminal.is_online as u8 as f32,
        terminal.card_present as u8 as f32,
        (!customer.known_merchants.contains(&merchant.id)) as u8 as f32,
        mcc_risk(merchant.mcc),
        clamp(merchant.avg_amount / MAX_MERCHANT_AVG_AMOUNT),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Customer, Merchant, Terminal, Transaction, TransactionPayload};
    use chrono::DateTime;

    fn legit_payload() -> TransactionPayload<'static> {
        TransactionPayload {
            transaction: Transaction {
                amount: 41.12,
                installments: 2,
                requested_at: "2026-03-11T18:45:53Z"
                    .parse::<DateTime<chrono::Utc>>()
                    .unwrap(),
            },
            customer: Customer {
                avg_amount: 82.24,
                tx_count_24h: 3,
                known_merchants: vec!["MERC-003", "MERC-016"],
            },
            merchant: Merchant {
                id: "MERC-016",
                mcc: "5411",
                avg_amount: 60.25,
            },
            terminal: Terminal {
                is_online: false,
                card_present: true,
                km_from_home: 29.23,
            },
            last_transaction: None,
        }
    }

    fn fraud_payload() -> TransactionPayload<'static> {
        TransactionPayload {
            transaction: Transaction {
                amount: 9505.97,
                installments: 10,
                requested_at: "2026-03-14T05:15:12Z"
                    .parse::<DateTime<chrono::Utc>>()
                    .unwrap(),
            },
            customer: Customer {
                avg_amount: 81.28,
                tx_count_24h: 20,
                known_merchants: vec!["MERC-008", "MERC-007", "MERC-005"],
            },
            merchant: Merchant {
                id: "MERC-068",
                mcc: "7802",
                avg_amount: 54.86,
            },
            terminal: Terminal {
                is_online: false,
                card_present: true,
                km_from_home: 952.27,
            },
            last_transaction: None,
        }
    }

    #[test]
    fn test_vectorize_legit_transaction() {
        let v = vectorize(&legit_payload());
        let expected = [
            0.0041, 0.1667, 0.05, 0.7826, 0.3333, -1.0, -1.0, 0.0292, 0.15, 0.0, 1.0, 0.0, 0.15,
            0.006,
        ];
        for (i, (got, exp)) in v.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 0.001,
                "index {i}: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn test_vectorize_fraud_transaction() {
        let v = vectorize(&fraud_payload());
        let expected = [
            0.9506, 0.8333, 1.0, 0.2174, 0.8333, -1.0, -1.0, 0.9523, 1.0, 0.0, 1.0, 1.0, 0.75,
            0.0055,
        ];
        for (i, (got, exp)) in v.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 0.001,
                "index {i}: got {got}, expected {exp}"
            );
        }
    }
}
