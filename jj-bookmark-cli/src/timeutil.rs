//! 日期时间工具：零依赖的 epoch 毫秒 ↔ 公历换算 + JST 串生成 + ISO8601(UTC) 解析。
//!
//! 设计取舍：JST = UTC+9，日本全年无夏令时，偏移恒定，故所有 `*_jst` 派生值都是
//! 「主字段(epoch 毫秒) + 9h → 公历」的无歧义换算，无需引入重型日期库。
//! 公历换算用 Howard Hinnant 的 `days_from_civil` / `civil_from_days` 算法
//! （proleptic Gregorian，公开领域，精确，见单元测试对齐 data-model 示例）。

use anyhow::{Context, Result, anyhow};

const JST_OFFSET_SECS: i64 = 9 * 3600;

/// 1970-01-01 起的天数（可为负）。proleptic Gregorian。
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// 天数（1970-01-01 起）→ (year, month, day)。
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
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

/// 解析 ISO 8601 时间戳 → epoch 毫秒。
///
/// 面向 raindrop 导出格式 `YYYY-MM-DDTHH:MM:SS[.fff]Z`；同时容错处理数字时区
/// 偏移 `±HH:MM` / `±HHMM`，归一化到 UTC。
pub fn parse_iso8601_to_millis(s: &str) -> Result<i64> {
    let s = s.trim();
    let (date, time_tz) = s
        .split_once('T')
        .or_else(|| s.split_once(' '))
        .ok_or_else(|| anyhow!("timestamp missing date/time separator: {s:?}"))?;

    // 日期
    let mut dp = date.split('-');
    let y = next_num(&mut dp, s)?;
    let mo = next_num(&mut dp, s)?;
    let d = next_num(&mut dp, s)?;

    // 拆分时区
    let (time, offset_secs) = split_offset(time_tz).with_context(|| format!("failed to parse time zone: {s:?}"))?;

    // 时:分:秒[.毫秒]
    let mut tp = time.split(':');
    let hh = next_num(&mut tp, s)?;
    let mm = next_num(&mut tp, s)?;
    let sec_part = tp
        .next()
        .ok_or_else(|| anyhow!("timestamp missing seconds: {s:?}"))?;
    let (ss, millis) = match sec_part.split_once('.') {
        Some((sec, frac)) => (parse_i64(sec, s)?, parse_frac_millis(frac)),
        None => (parse_i64(sec_part, s)?, 0),
    };

    let days = days_from_civil(y, mo, d);
    let total_secs = days * 86400 + hh * 3600 + mm * 60 + ss - offset_secs;
    Ok(total_secs * 1000 + millis)
}

/// 从「时间+时区」串里剥离时区，返回 (纯时间, 偏移秒)。
fn split_offset(time_tz: &str) -> Result<(&str, i64)> {
    if let Some(t) = time_tz.strip_suffix('Z').or_else(|| time_tz.strip_suffix('z')) {
        return Ok((t, 0));
    }
    // 找符号位（跳过首字符，避免误判负号开头的异常输入）
    let bytes = time_tz.as_bytes();
    for (i, &b) in bytes.iter().enumerate().skip(1) {
        if b == b'+' || b == b'-' {
            let (time, off) = time_tz.split_at(i);
            let sign = if b == b'+' { 1 } else { -1 };
            let off = &off[1..];
            let (oh, om) = match off.split_once(':') {
                Some((h, m)) => (h.parse::<i64>()?, m.parse::<i64>()?),
                None if off.len() == 4 => (off[..2].parse::<i64>()?, off[2..].parse::<i64>()?),
                None => (off.parse::<i64>()?, 0),
            };
            return Ok((time, sign * (oh * 3600 + om * 60)));
        }
    }
    Ok((time_tz, 0)) // 无时区标记，按 UTC 处理
}

fn parse_frac_millis(frac: &str) -> i64 {
    let mut buf = [b'0'; 3];
    for (i, b) in frac.bytes().take(3).enumerate() {
        buf[i] = b;
    }
    std::str::from_utf8(&buf).ok().and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn next_num<'a, I: Iterator<Item = &'a str>>(it: &mut I, ctx: &str) -> Result<i64> {
    let part = it.next().ok_or_else(|| anyhow!("not enough timestamp fields: {ctx:?}"))?;
    parse_i64(part, ctx)
}

fn parse_i64(s: &str, ctx: &str) -> Result<i64> {
    s.trim()
        .parse()
        .with_context(|| format!("failed to parse timestamp number ({s:?} in {ctx:?})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 1, 1), 10957);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10957), (2000, 1, 1));
    }

    #[test]
    fn civil_roundtrip_range() {
        // 覆盖多年逐日往返一致
        for z in -100_000..100_000 {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m as i64, d as i64), z, "z={z}");
        }
    }

    // 测试向量取自 data-model.md §4 示例（用数字主字段核算，示例 #2 原派生串曾误差 1 天，
    // 已在文档同步订正；数字主字段是 source of truth）。用 python stdlib 独立复核过。
    #[test]
    fn jst_matches_data_model_examples() {
        assert_eq!(epoch_millis_to_jst(1774873951941), "2026-03-30 21:32:31+09:00");
        assert_eq!(epoch_millis_to_jst(1784106720000), "2026-07-15 18:12:00+09:00");
        assert_eq!(epoch_millis_to_jst(1767052800000), "2025-12-30 09:00:00+09:00");
    }

    #[test]
    fn parse_raindrop_iso() {
        assert_eq!(parse_iso8601_to_millis("2026-03-30T12:32:31.941Z").unwrap(), 1774873951941);
        // 无毫秒
        assert_eq!(parse_iso8601_to_millis("2025-12-30T00:00:00Z").unwrap(), 1767052800000);
        // 带数字偏移，归一化到 UTC
        assert_eq!(
            parse_iso8601_to_millis("2026-03-30T21:32:31.941+09:00").unwrap(),
            1774873951941
        );
    }

    #[test]
    fn parse_then_format_roundtrip() {
        let ms = parse_iso8601_to_millis("2026-03-30T12:32:31.941Z").unwrap();
        assert_eq!(epoch_millis_to_jst(ms), "2026-03-30 21:32:31+09:00");
    }

    #[test]
    fn frac_padding() {
        assert_eq!(parse_frac_millis("9"), 900);
        assert_eq!(parse_frac_millis("94"), 940);
        assert_eq!(parse_frac_millis("941"), 941);
        assert_eq!(parse_frac_millis("9415"), 941); // 截断到毫秒
    }
}
