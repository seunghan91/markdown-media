//! Korean Natural Language Date Parser
//!
//! Converts Korean date expressions to NaiveDate
//! Examples: "최근 3개월", "작년", "다음주 화요일", "시행일로부터 30일"

use chrono::{Datelike, Local, NaiveDate, Weekday};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateResult {
    pub date: String,
    pub end_date: Option<String>,
    pub format: DateFormat,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    Absolute,
    Relative,
    Duration,
    Legal,
    Weekday,
}

lazy_static! {
    static ref RE_ABS_KOREAN: Regex =
        Regex::new(r"(\d{4})년\s*(\d{1,2})월\s*(\d{1,2})일").unwrap();
    static ref RE_ABS_DOT: Regex = Regex::new(r"(\d{4})\.(\d{1,2})\.(\d{1,2})").unwrap();
    static ref RE_OFFSET: Regex =
        Regex::new(r"(\d+)\s*(일|주|달|개월|년)\s*(전|후|이내|뒤|내)").unwrap();
    static ref RE_WEEKDAY: Regex =
        Regex::new(r"(이번|다음|지난|저번|다다음)주\s*(월요일|화요일|수요일|목요일|금요일|토요일|일요일)")
            .unwrap();
    static ref RE_RECENT: Regex =
        Regex::new(r"최근\s*(\d+)?\s*(일|주|달|개월|년|분기)").unwrap();
    static ref RE_QUARTER: Regex =
        Regex::new(r"(?:(\d{4})년\s*)?(상반기|하반기|([1-4])분기)").unwrap();
    static ref RE_LEGAL_PERIOD: Regex = Regex::new(
        r"(시행일|공포일|개정일|기준일)(?:로부터)?\s*(\d+)\s*(일|개월|년)\s*(이내|이후|이상)?"
    )
    .unwrap();
}

pub struct KoreanDateParser {
    reference: NaiveDate,
}

impl KoreanDateParser {
    pub fn new(reference: NaiveDate) -> Self {
        Self { reference }
    }

    pub fn today() -> Self {
        Self {
            reference: Local::now().date_naive(),
        }
    }

    pub fn parse(&self, text: &str) -> Option<DateResult> {
        let text = text.trim();
        self.parse_absolute(text)
            .or_else(|| self.parse_named_relative(text))
            .or_else(|| self.parse_offset(text))
            .or_else(|| self.parse_weekday(text))
            .or_else(|| self.parse_recent(text))
            .or_else(|| self.parse_quarter(text))
            .or_else(|| self.parse_legal_period(text))
    }

    fn parse_absolute(&self, text: &str) -> Option<DateResult> {
        if let Some(caps) = RE_ABS_KOREAN.captures(text) {
            let y: i32 = caps[1].parse().ok()?;
            let m: u32 = caps[2].parse().ok()?;
            let d: u32 = caps[3].parse().ok()?;
            let date = NaiveDate::from_ymd_opt(y, m, d)?;
            return Some(DateResult {
                date: fmt(date),
                end_date: None,
                format: DateFormat::Absolute,
                confidence: 0.95,
            });
        }
        if let Some(caps) = RE_ABS_DOT.captures(text) {
            let y: i32 = caps[1].parse().ok()?;
            let m: u32 = caps[2].parse().ok()?;
            let d: u32 = caps[3].parse().ok()?;
            let date = NaiveDate::from_ymd_opt(y, m, d)?;
            return Some(DateResult {
                date: fmt(date),
                end_date: None,
                format: DateFormat::Absolute,
                confidence: 0.90,
            });
        }
        None
    }

    fn parse_named_relative(&self, text: &str) -> Option<DateResult> {
        let r = self.reference;
        match text {
            "오늘" => Some(dr(fmt(r), DateFormat::Relative)),
            "내일" => Some(dr(
                fmt(r + chrono::Duration::days(1)),
                DateFormat::Relative,
            )),
            "모레" => Some(dr(
                fmt(r + chrono::Duration::days(2)),
                DateFormat::Relative,
            )),
            "어제" => Some(dr(
                fmt(r - chrono::Duration::days(1)),
                DateFormat::Relative,
            )),
            "그제" | "그저께" => Some(dr(
                fmt(r - chrono::Duration::days(2)),
                DateFormat::Relative,
            )),
            "올해" => {
                let s = NaiveDate::from_ymd_opt(r.year(), 1, 1)?;
                let e = NaiveDate::from_ymd_opt(r.year(), 12, 31)?;
                Some(DateResult {
                    date: fmt(s),
                    end_date: Some(fmt(e)),
                    format: DateFormat::Duration,
                    confidence: 0.95,
                })
            }
            "작년" => {
                let y = r.year() - 1;
                let s = NaiveDate::from_ymd_opt(y, 1, 1)?;
                let e = NaiveDate::from_ymd_opt(y, 12, 31)?;
                Some(DateResult {
                    date: fmt(s),
                    end_date: Some(fmt(e)),
                    format: DateFormat::Duration,
                    confidence: 0.95,
                })
            }
            "이번달" => {
                let s = NaiveDate::from_ymd_opt(r.year(), r.month(), 1)?;
                Some(DateResult {
                    date: fmt(s),
                    end_date: None,
                    format: DateFormat::Duration,
                    confidence: 0.90,
                })
            }
            "다음달" => {
                let d = add_months(r, 1);
                let s = NaiveDate::from_ymd_opt(d.year(), d.month(), 1)?;
                Some(DateResult {
                    date: fmt(s),
                    end_date: None,
                    format: DateFormat::Duration,
                    confidence: 0.90,
                })
            }
            "지난달" => {
                let d = add_months(r, -1);
                let s = NaiveDate::from_ymd_opt(d.year(), d.month(), 1)?;
                Some(DateResult {
                    date: fmt(s),
                    end_date: None,
                    format: DateFormat::Duration,
                    confidence: 0.90,
                })
            }
            _ => None,
        }
    }

    fn parse_offset(&self, text: &str) -> Option<DateResult> {
        let caps = RE_OFFSET.captures(text)?;
        let n: i64 = caps[1].parse().ok()?;
        let unit = &caps[2];
        let dir = &caps[3];
        let mul: i64 = match dir {
            "전" | "이내" | "내" => -1,
            "후" | "뒤" => 1,
            _ => return None,
        };

        let date = match unit {
            "일" => self.reference + chrono::Duration::days(n * mul),
            "주" => self.reference + chrono::Duration::weeks(n * mul),
            "달" | "개월" => add_months(self.reference, (n * mul) as i32),
            "년" => NaiveDate::from_ymd_opt(
                self.reference.year() + (n * mul) as i32,
                self.reference.month(),
                self.reference.day(),
            )?,
            _ => return None,
        };
        Some(dr(fmt(date), DateFormat::Relative))
    }

    fn parse_weekday(&self, text: &str) -> Option<DateResult> {
        let caps = RE_WEEKDAY.captures(text)?;
        let week_ref = &caps[1];
        let day_name = &caps[2];

        let target = match day_name {
            "월요일" => Weekday::Mon,
            "화요일" => Weekday::Tue,
            "수요일" => Weekday::Wed,
            "목요일" => Weekday::Thu,
            "금요일" => Weekday::Fri,
            "토요일" => Weekday::Sat,
            "일요일" => Weekday::Sun,
            _ => return None,
        };

        let week_off: i64 = match week_ref {
            "이번" => 0,
            "다음" => 1,
            "지난" | "저번" => -1,
            "다다음" => 2,
            _ => return None,
        };

        let cur = self.reference.weekday().num_days_from_monday() as i64;
        let tgt = target.num_days_from_monday() as i64;
        let diff = tgt - cur + week_off * 7;
        let date = self.reference + chrono::Duration::days(diff);
        Some(DateResult {
            date: fmt(date),
            end_date: None,
            format: DateFormat::Weekday,
            confidence: 0.92,
        })
    }

    fn parse_recent(&self, text: &str) -> Option<DateResult> {
        let caps = RE_RECENT.captures(text)?;
        let n: i64 = caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);
        let unit = &caps[2];

        let start = match unit {
            "일" => self.reference - chrono::Duration::days(n),
            "주" => self.reference - chrono::Duration::weeks(n),
            "달" | "개월" => add_months(self.reference, -(n as i32)),
            "년" => NaiveDate::from_ymd_opt(
                self.reference.year() - n as i32,
                self.reference.month(),
                self.reference.day(),
            )?,
            "분기" => add_months(self.reference, -(n as i32 * 3)),
            _ => return None,
        };
        Some(DateResult {
            date: fmt(start),
            end_date: Some(fmt(self.reference)),
            format: DateFormat::Duration,
            confidence: 0.85,
        })
    }

    fn parse_quarter(&self, text: &str) -> Option<DateResult> {
        let caps = RE_QUARTER.captures(text)?;
        let year = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(self.reference.year());

        let (sm, em) = if let Some(q) = caps.get(3) {
            match q.as_str() {
                "1" => (1, 3),
                "2" => (4, 6),
                "3" => (7, 9),
                "4" => (10, 12),
                _ => return None,
            }
        } else {
            match &caps[2] {
                "상반기" => (1, 6),
                "하반기" => (7, 12),
                _ => return None,
            }
        };

        let start = NaiveDate::from_ymd_opt(year, sm, 1)?;
        let end = NaiveDate::from_ymd_opt(year, em, last_day(year, em))?;
        Some(DateResult {
            date: fmt(start),
            end_date: Some(fmt(end)),
            format: DateFormat::Duration,
            confidence: 0.92,
        })
    }

    fn parse_legal_period(&self, text: &str) -> Option<DateResult> {
        let caps = RE_LEGAL_PERIOD.captures(text)?;
        let n: i64 = caps[2].parse().ok()?;
        let unit = &caps[3];

        let date = match unit {
            "일" => self.reference + chrono::Duration::days(n),
            "개월" => add_months(self.reference, n as i32),
            "년" => NaiveDate::from_ymd_opt(
                self.reference.year() + n as i32,
                self.reference.month(),
                self.reference.day(),
            )?,
            _ => return None,
        };
        Some(DateResult {
            date: fmt(date),
            end_date: None,
            format: DateFormat::Legal,
            confidence: 0.88,
        })
    }
}

fn fmt(d: NaiveDate) -> String {
    d.format("%Y%m%d").to_string()
}

fn dr(date: String, format: DateFormat) -> DateResult {
    DateResult {
        date,
        end_date: None,
        format,
        confidence: 0.95,
    }
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total = date.year() * 12 + date.month() as i32 - 1 + months;
    let y = total / 12;
    let m = (total % 12 + 1) as u32;
    let d = date.day().min(last_day(y, m));
    NaiveDate::from_ymd_opt(y, m, d).unwrap_or(date)
}

fn last_day(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(year, month + 1, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap())
        .pred_opt()
        .unwrap()
        .day()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> KoreanDateParser {
        KoreanDateParser::new(NaiveDate::from_ymd_opt(2026, 4, 2).unwrap())
    }

    #[test]
    fn test_absolute_korean() {
        let r = p().parse("2024년 3월 1일").unwrap();
        assert_eq!(r.date, "20240301");
        assert_eq!(r.format, DateFormat::Absolute);
    }

    #[test]
    fn test_absolute_dot() {
        let r = p().parse("2024.12.31").unwrap();
        assert_eq!(r.date, "20241231");
    }

    #[test]
    fn test_today() {
        assert_eq!(p().parse("오늘").unwrap().date, "20260402");
    }

    #[test]
    fn test_yesterday() {
        assert_eq!(p().parse("어제").unwrap().date, "20260401");
    }

    #[test]
    fn test_tomorrow() {
        assert_eq!(p().parse("내일").unwrap().date, "20260403");
    }

    #[test]
    fn test_offset_days() {
        assert_eq!(p().parse("3일 전").unwrap().date, "20260330");
        assert_eq!(p().parse("5일 후").unwrap().date, "20260407");
    }

    #[test]
    fn test_offset_months() {
        assert_eq!(p().parse("3개월 전").unwrap().date, "20260102");
        assert_eq!(p().parse("6개월 후").unwrap().date, "20261002");
    }

    #[test]
    fn test_weekday() {
        // 2026-04-02 = Thursday
        assert_eq!(p().parse("이번주 월요일").unwrap().date, "20260330");
        assert_eq!(p().parse("다음주 화요일").unwrap().date, "20260407");
    }

    #[test]
    fn test_recent() {
        let r = p().parse("최근 3개월").unwrap();
        assert_eq!(r.date, "20260102");
        assert_eq!(r.end_date, Some("20260402".to_string()));
    }

    #[test]
    fn test_quarter() {
        let r = p().parse("2024년 1분기").unwrap();
        assert_eq!(r.date, "20240101");
        assert_eq!(r.end_date, Some("20240331".to_string()));
    }

    #[test]
    fn test_half_year() {
        let r = p().parse("상반기").unwrap();
        assert_eq!(r.date, "20260101");
        assert_eq!(r.end_date, Some("20260630".to_string()));
    }

    #[test]
    fn test_last_year() {
        let r = p().parse("작년").unwrap();
        assert_eq!(r.date, "20250101");
        assert_eq!(r.end_date, Some("20251231".to_string()));
    }

    #[test]
    fn test_legal_period() {
        let r = p().parse("시행일로부터 30일").unwrap();
        assert_eq!(r.date, "20260502");
        assert_eq!(r.format, DateFormat::Legal);
    }

    #[test]
    fn test_none_for_garbage() {
        assert!(p().parse("아무말대잔치").is_none());
        assert!(p().parse("").is_none());
    }
}
