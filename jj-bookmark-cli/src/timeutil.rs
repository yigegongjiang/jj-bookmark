//! 日期时间工具：零依赖的 epoch 毫秒 → JST 可读串。
//!
//! 设计取舍：JST = UTC+9，日本全年无夏令时，偏移恒定，故所有 `*_jst` 派生值都是
//! 「主字段(epoch 毫秒) + 9h → 公历」的无歧义换算，无需引入重型日期库。
//! 公历换算用 Howard Hinnant 的 `civil_from_days` 算法
//! （proleptic Gregorian，公开领域，精确，见单元测试对齐 data-model 示例）。

const JST_OFFSET_SECS: i64 = 9 * 3600;

/// 天数（1970-01-01 起）→ (year, month, day)。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// epoch 毫秒 → 日本时间可读串 `YYYY-MM-DD HH:MM:SS+09:00`。
pub fn epoch_millis_to_jst(ms: i64) -> String {
    let secs = ms.div_euclid(1000) + JST_OFFSET_SECS;
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400); // [0, 86399]
    let (y, mo, d) = civil_from_days(days);
    let (hh, mm, ss) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    format!("{y:04}-{mo:02}-{d:02} {hh:02}:{mm:02}:{ss:02}+09:00")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试向量取自 data-model.md §4 示例（用数字主字段核算，示例 #2 原派生串曾误差 1 天，
    // 已在文档同步订正；数字主字段是 source of truth）。用 python stdlib 独立复核过。
    #[test]
    fn jst_matches_data_model_examples() {
        assert_eq!(epoch_millis_to_jst(1774873951941), "2026-03-30 21:32:31+09:00");
        assert_eq!(epoch_millis_to_jst(1784106720000), "2026-07-15 18:12:00+09:00");
        assert_eq!(epoch_millis_to_jst(1767052800000), "2025-12-30 09:00:00+09:00");
    }
}
