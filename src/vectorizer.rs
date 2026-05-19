use std::collections::HashMap;

use crate::models::{Normalization, TransactionRequest};

pub fn vectorize(
    tx: &TransactionRequest,
    normalization: &Normalization,
    mcc_risk: &HashMap<String, f64>,
) -> [f64; 14] {
    todo!()
}
