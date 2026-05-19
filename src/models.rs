use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TransactionRequest {
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

#[derive(Deserialize)]
pub struct Normalization {
    pub max_ammount: f64, // teto para transaction.amount; valores acima de 10.000 são limitados a 1.0
    pub max_installments: u32, // teto para transaction.installments (12 parcelas equivalem a 1.0)
    pub amount_vs_avg_ratio: f64, // divisor da razão amount / customer.avg_amount; 10× a média equivale a 1..
    pub max_minutes: u32, // janela de tempo para minutes_since_last_tx; 1.440 min correspondem a 24h
    pub max_km: f64,      // teto de distância (km) para km_from_home e km_from_last_tx
    pub max_tx_count_24h: f64, // teto para customer.tx_count_24h; 20 transações ou mais nas últimas 24h são limitadas a 1.0
    pub max_merchant_avg_amount: f64, // teto para o ticket médio do comerciante
}
