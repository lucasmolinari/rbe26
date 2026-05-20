use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TransactionRequest {
    #[allow(unused)]
    pub id: String,
    pub transaction: TransactionData,
    pub customer: CustomerData,
    pub merchant: MerchantData,
    pub terminal: TerminalData,
    pub last_transaction: Option<LastTransaction>,
}

#[derive(Deserialize)]
pub struct TransactionData {
    pub amount: f64,
    pub installments: u32,
    pub requested_at: String,
}

#[derive(Deserialize)]
pub struct CustomerData {
    pub avg_amount: f64,
    pub tx_count_24h: u32,
    pub known_merchants: Vec<String>,
}

#[derive(Deserialize)]
pub struct MerchantData {
    pub id: String,
    pub mcc: String,
    pub avg_amount: f64,
}

#[derive(Deserialize)]
pub struct TerminalData {
    pub is_online: bool,
    pub card_present: bool,
    pub km_from_home: f64,
}

#[derive(Deserialize)]
pub struct LastTransaction {
    pub timestamp: String,
    pub km_from_current: f64,
}

#[derive(Serialize)]
pub struct FraudResponse {
    pub approved: bool,
    pub fraud_score: f64,
}
