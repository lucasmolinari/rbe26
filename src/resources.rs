use qdrant_client::Qdrant;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Normalization {
    pub max_amount: f64,
    pub max_installments: f64,
    pub amount_vs_avg_ratio: f64,
    pub max_minutes: f64,
    pub max_km: f64,
    pub max_tx_count_24h: f64,
    pub max_merchant_avg_amount: f64,
}

pub struct Resources {
    pub qdrant_client: Qdrant,
    pub normalization: Normalization,
    pub mcc_risk: HashMap<String, f64>,
}

impl std::fmt::Debug for Resources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resources")
            .field("normalization", &self.normalization)
            .field("mcc_risk", &self.mcc_risk)
            .finish_non_exhaustive()
    }
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

        let qdrant_client = Qdrant::from_url(
            &std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://qdrant:6334".into()),
        )
        .build()
        .map_err(|e| format!("failed to create qdrant client: {}", e))?;

        Ok(Self {
            qdrant_client,
            normalization,
            mcc_risk,
        })
    }
}
