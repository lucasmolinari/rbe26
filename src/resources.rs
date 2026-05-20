use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, io::Read};

use flate2::read::GzDecoder;

#[derive(Deserialize, Debug)]
pub struct Reference {
    pub vector: [f64; 14],
    #[serde(rename = "label", deserialize_with = "deserialize_label")]
    pub is_fraud: bool,
}

#[derive(Deserialize, Debug)]
pub struct Normalization {
    pub max_amount: f64, // teto para transaction.amount; valores acima de 10.000 são limitados a 1.0
    pub max_installments: f64, // teto para transaction.installments (12 parcelas equivalem a 1.0)
    pub amount_vs_avg_ratio: f64, // divisor da razão amount / customer.avg_amount; 10× a média equivale a 1..
    pub max_minutes: f64, // janela de tempo para minutes_since_last_tx; 1.440 min correspondem a 24h
    pub max_km: f64,      // teto de distância (km) para km_from_home e km_from_last_tx
    pub max_tx_count_24h: f64, // teto para customer.tx_count_24h; 20 transações ou mais nas últimas 24h são limitadas a 1.0
    pub max_merchant_avg_amount: f64, // teto para o ticket médio do comerciante
}

#[derive(Debug)]
pub struct Resources {
    pub references: Vec<Reference>,
    pub normalization: Normalization,
    pub mcc_risk: HashMap<String, f64>,
}

impl Resources {
    pub fn new() -> Result<Self, String> {
        let normalization = std::fs::read_to_string("resources/normalization.json")
            .map_err(|e| format!("failed to read normalization.json: {}", e))?;
        let normalization: Normalization = serde_json::from_str(&normalization)
            .map_err(|e| format!("failed to parse normalization.json: {}", e))?;

        let mcc_risk = std::fs::read_to_string("resources/mcc_risk.json")
            .map_err(|e| format!("failed to read mcc_risk.json: {}", e))?;
        let mcc_risk: HashMap<String, f64> = serde_json::from_str(&mcc_risk)
            .map_err(|e| format!("failed to parse mcc_risk.json: {}", e))?;

        let file = std::fs::File::open("resources/references.json.gz")
            .map_err(|e| format!("failed to open references.json.gz: {}", e))?;
        let mut decoder = GzDecoder::new(file);
        let mut contents = String::new();
        decoder
            .read_to_string(&mut contents)
            .map_err(|e| format!("failed to decompress references.json.gz: {}", e))?;

        let references: Vec<Reference> = serde_json::from_str(&contents)
            .map_err(|e| format!("failed to parse references.json.gz contents: {}", e))?;

        Ok(Self {
            references,
            normalization,
            mcc_risk,
        })
    }
}

fn deserialize_label<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s == "fraud")
}
