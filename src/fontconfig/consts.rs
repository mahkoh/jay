#![allow(dead_code)]

cenum! {
    _FcMatchKind, FC_MATCH_KINDS;

    FC_MATCH_PATTERN = 0,
    FC_MATCH_FONT = 1,
    FC_MATCH_SCAN = 2,
    FC_MATCH_KIND_END = 3,
}

cenum! {
    _FcResult, FC_RESULTS;

    FC_RESULT_MATCH = 0,
    FC_RESULT_NO_MATCH = 1,
    FC_RESULT_TYPE_MISMATCH = 2,
    FC_RESULT_NO_ID = 3,
    FC_RESULT_OUT_OF_MEMORY = 4,
}
