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

pub const BUCKET_COUNT: usize =
    AMOUNT_BINS * RATIO_BINS * HOUR_BINS * HOME_BINS * TX_BINS * MCC_BINS * 2 * 2 * 2 * 2;

pub fn bucket_key_from_quantized(vector: &[i16; VECTOR_DIMS]) -> usize {
    bucket_key(
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

pub fn bucket_key(
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

pub fn decode_bucket_key(key: usize) -> [usize; 10] {
    let unknown = key % BOOL_STATES;
    let key = key / BOOL_STATES;
    let card = key % BOOL_STATES;
    let key = key / BOOL_STATES;
    let online = key % BOOL_STATES;
    let key = key / BOOL_STATES;
    let last = key % LAST_STATES;
    let key = key / LAST_STATES;
    let mcc = key % MCC_BINS;
    let key = key / MCC_BINS;
    let tx = key % TX_BINS;
    let key = key / TX_BINS;
    let home = key % HOME_BINS;
    let key = key / HOME_BINS;
    let hour = key % HOUR_BINS;
    let key = key / HOUR_BINS;
    let ratio = key % RATIO_BINS;
    let amount = key / RATIO_BINS;

    [
        amount, ratio, hour, home, tx, mcc, last, online, card, unknown,
    ]
}

fn bin01(value: i16, bins: usize) -> usize {
    let clamped = value.clamp(0, 32767) as usize;
    ((clamped * bins) / 32768).min(bins - 1)
}

fn bin_bool(value: i16) -> usize {
    usize::from(value > 16384)
}
