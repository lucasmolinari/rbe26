pub const VECTOR_DIMS: usize = 14;
pub const VECTOR_BYTES: usize = VECTOR_DIMS * std::mem::size_of::<i16>();

pub const AMOUNT_BINS: usize = 16;
pub const RATIO_BINS: usize = 16;
pub const HOUR_BINS: usize = 8;
pub const HOME_BINS: usize = 16;
pub const TX_BINS: usize = 8;
pub const MCC_BINS: usize = 4;
const LAST_STATES: usize = 2;
const BOOL_STATES: usize = 2;

pub const BLOCK_SIZE: usize = 64;
pub const PARTITION_COUNT: usize = AMOUNT_BINS * RATIO_BINS * HOUR_BINS * MCC_BINS * 2 * 2 * 2 * 2;

pub fn sort_key_from_quantized(vector: &[i16; VECTOR_DIMS]) -> usize {
    sort_key(
        bin01(vector[0], AMOUNT_BINS),
        bin01(vector[2], RATIO_BINS),
        bin01(vector[3], HOUR_BINS),
        bin01(vector[7], HOME_BINS),
        bin01(vector[8], TX_BINS),
        bin01(vector[12], MCC_BINS),
        usize::from(vector[5] >= 0),
        bin_bool(vector[9]),
        bin_bool(vector[10]),
        bin_bool(vector[11]),
    )
}

fn sort_key(
    amount: usize,
    ratio: usize,
    hour: usize,
    home: usize,
    tx: usize,
    mcc: usize,
    last: usize,
    online: usize,
    card: usize,
    unknown: usize,
) -> usize {
    let mut key = amount;
    key = key * RATIO_BINS + ratio;
    key = key * HOUR_BINS + hour;
    key = key * HOME_BINS + home;
    key = key * TX_BINS + tx;
    key = key * MCC_BINS + mcc;
    key = key * LAST_STATES + last;
    key = key * BOOL_STATES + online;
    key = key * BOOL_STATES + card;
    key = key * BOOL_STATES + unknown;
    key
}

pub fn partition_key_from_quantized(vector: &[i16; VECTOR_DIMS]) -> usize {
    partition_key(
        bin01(vector[0], AMOUNT_BINS),
        bin01(vector[2], RATIO_BINS),
        bin01(vector[3], HOUR_BINS),
        bin01(vector[12], MCC_BINS),
        usize::from(vector[5] >= 0),
        bin_bool(vector[9]),
        bin_bool(vector[10]),
        bin_bool(vector[11]),
    )
}

fn partition_key(
    amount: usize,
    ratio: usize,
    hour: usize,
    mcc: usize,
    last: usize,
    online: usize,
    card: usize,
    unknown: usize,
) -> usize {
    let mut key = amount;
    key = key * RATIO_BINS + ratio;
    key = key * HOUR_BINS + hour;
    key = key * MCC_BINS + mcc;
    key = key * LAST_STATES + last;
    key = key * BOOL_STATES + online;
    key = key * BOOL_STATES + card;
    key = key * BOOL_STATES + unknown;
    key
}

fn bin01(value: i16, bins: usize) -> usize {
    let clamped = value.clamp(0, 32767) as usize;
    ((clamped * bins) / 32768).min(bins - 1)
}

fn bin_bool(value: i16) -> usize {
    usize::from(value > 16384)
}
