use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Transaction {
    pub amount: f32,
    pub installments: u8,
    pub requested_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct Customer<'a> {
    pub avg_amount: f32,
    pub tx_count_24h: u8,
    #[serde(borrow)]
    pub known_merchants: Vec<&'a str>,
}

#[derive(Deserialize)]
pub struct Merchant<'a> {
    pub id: &'a str,
    pub mcc: &'a str,
    pub avg_amount: f32,
}

#[derive(Deserialize)]
pub struct Terminal {
    pub is_online: bool,
    pub card_present: bool,
    pub km_from_home: f32,
}

#[derive(Deserialize)]
pub struct LastTransaction {
    pub timestamp: DateTime<Utc>,
    pub km_from_current: f32,
}

#[derive(Deserialize)]
pub struct TransactionPayload<'a> {
    pub transaction: Transaction,
    #[serde(borrow)]
    pub customer: Customer<'a>,
    #[serde(borrow)]
    pub merchant: Merchant<'a>,
    pub terminal: Terminal,
    pub last_transaction: Option<LastTransaction>,
}

#[derive(Serialize)]
pub struct FraudResponse {
    pub approved: bool,
    pub fraud_score: f32,
}
