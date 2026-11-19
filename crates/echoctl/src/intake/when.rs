//! Dates said in a search query ("12 November", "2026-11-12", "last week", "12 листопада", "на прошлой неделе")
//! become its since/until filter, and leave the words it searches. Conservative: a day with its month (any year
//! form), an ISO date, or a named relative day or week; a month alone, or a number alone, is never a date.

use chrono::{Datelike, Duration, NaiveDate};

/// A date range said in a query: its first and last day (YYYY-MM-DD) and the words that said it.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub since: String,
    pub until: String,
    pub words: String,
}

/// Month names (English, Ukrainian, Russian; with their usual forms) → month.
const MONTHS: [(&str, u32); 12] = [
    ("january jan січня січень января январь", 1),
    ("february feb лютого лютий февраля февраль", 2),
    ("march mar березня березень марта март", 3),
    ("april apr квітня квітень апреля апрель", 4),
    ("may травня травень мая май", 5),
    ("june jun червня червень июня июнь", 6),
    ("july jul липня липень июля июль", 7),
    ("august aug серпня серпень августа август", 8),
    ("september sep sept вересня вересень сентября сентябрь", 9),
    ("october oct жовтня жовтень октября октябрь", 10),
    ("november nov листопада листопад ноября ноябрь", 11),
    ("december dec грудня грудень декабря декабрь", 12),
];

fn month(w: &str) -> Option<u32> {
    let w = w.trim_end_matches('.');
    MONTHS.iter().find(|(names, _)| names.split(' ').any(|n| n == w)).map(|(_, m)| *m)
}

/// A day of the month: "12", "12th", "1st", "12-го".
fn day(w: &str) -> Option<u32> {
    let digits: String = w.chars().take_while(char::is_ascii_digit).collect();
    let rest = &w[digits.len()..];
    let d: u32 = digits.parse().ok()?;
    (["", "st", "nd", "rd", "th", "-го", "-е"].contains(&rest) && (1..=31).contains(&d)).then_some(d)
}

fn year(w: &str) -> Option<i32> {
    (w.len() == 4).then(|| w.parse().ok()).flatten().filter(|y| (2000..=2100).contains(y))
}

/// Relative days and weeks, as word sequences (lowercase) → (first day, last day) from today.
fn relative(words: &[&str], today: NaiveDate) -> Option<(usize, NaiveDate, NaiveDate)> {
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let spans: [(&[&str], NaiveDate, NaiveDate); 6] = [
        (&["today", "сьогодні", "сегодня"], today, today),
        (&["yesterday", "вчора", "вчера"], today - Duration::days(1), today - Duration::days(1)),
        (&["last week", "минулого тижня", "на минулому тижні", "на прошлой неделе", "прошлой неделе"], monday - Duration::days(7), monday - Duration::days(1)),
        (&["this week", "цього тижня", "на цьому тижні", "на этой неделе", "этой неделе"], monday, today),
        (&["past week", "last 7 days", "за тиждень", "за неделю"], today - Duration::days(6), today),
        (&["last month", "минулого місяця", "в прошлом месяце", "прошлом месяце"], {
            let first = today.with_day(1)?;
            let prev_last = first - Duration::days(1);
            prev_last.with_day(1)?
        }, today.with_day(1)? - Duration::days(1)),
    ];
    // The longest phrase first ("на прошлой неделе" before "прошлой неделе").
    let mut best: Option<(usize, NaiveDate, NaiveDate)> = None;
    for (phrases, a, b) in spans {
        for p in phrases {
            let n = p.split(' ').count();
            if words.len() >= n && words[..n].join(" ") == *p && best.is_none_or(|x| x.0 < n) {
                best = Some((n, a, b));
            }
        }
    }
    best
}

/// A day and month without a year: the latest such day not after today.
fn latest(m: u32, d: u32, today: NaiveDate) -> Option<NaiveDate> {
    let this = NaiveDate::from_ymd_opt(today.year(), m, d);
    match this {
        Some(x) if x <= today => Some(x),
        _ => NaiveDate::from_ymd_opt(today.year() - 1, m, d),
    }
}

/// An explicit date at the start of `words` (lowercase; `orig`: as written; `comma`: a comma right after the word):
/// how many words it takes and the day. An English month word ("may", "march") is a month only capitalised or in a
/// form only a date has: with a year ("3 may 2026") or a day and a comma ("may 3, …").
fn explicit(words: &[&str], orig: &[&str], comma: &[bool], today: NaiveDate) -> Option<(usize, NaiveDate)> {
    let w0 = *words.first()?;
    if let Ok(d) = NaiveDate::parse_from_str(w0, "%Y-%m-%d") {
        return Some((1, d));
    }
    let has_year = |n: usize| words.get(n).and_then(|w| year(w));
    let english_ok = |at: usize, dated: bool| !words[at].is_ascii() || orig[at].starts_with(|c: char| c.is_uppercase()) || dated;
    let with_year = |n: usize, m: u32, d: u32| -> Option<(usize, NaiveDate)> {
        match has_year(n) {
            Some(y) => NaiveDate::from_ymd_opt(y, m, d).map(|x| (n + 1, x)),
            None => latest(m, d, today).map(|x| (n, x)),
        }
    };
    // "12 November [2026]", "12 листопада", "12-го ноября"
    if let (Some(d), Some(m)) = (day(w0), words.get(1).and_then(|w| month(w))) {
        return english_ok(1, has_year(2).is_some()).then(|| with_year(2, m, d)).flatten();
    }
    // "November 12[, 2026]"
    if let (Some(m), Some(d)) = (month(w0), words.get(1).and_then(|w| day(w))) {
        return english_ok(0, has_year(2).is_some() || comma[1]).then(|| with_year(2, m, d)).flatten();
    }
    None
}

/// The dates said in `q` (all of them: their earliest and latest day), and the query without them.
pub fn in_query(q: &str, today: NaiveDate) -> Option<(Said, String)> {
    // Words with their byte ranges: letters, digits and the hyphens inside them; a trailing comma or dot ends one.
    let mut toks: Vec<(usize, usize, String)> = vec![];
    let mut start = None;
    for (i, ch) in q.char_indices().chain(std::iter::once((q.len(), ' '))) {
        let part = ch.is_alphanumeric() || ch == '-';
        match (part, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                toks.push((s, i, q[s..i].trim_matches('-').to_lowercase()));
                start = None;
            }
            _ => {}
        }
    }
    let words: Vec<&str> = toks.iter().map(|t| t.2.as_str()).collect();
    let orig: Vec<&str> = toks.iter().map(|t| q[t.0..t.1].trim_matches('-')).collect();
    let comma: Vec<bool> = toks.iter().map(|t| q[t.1..].starts_with(',')).collect();
    let mut found: Vec<(usize, usize, NaiveDate, NaiveDate)> = vec![];
    let mut i = 0;
    while i < words.len() {
        if let Some((n, a, b)) = relative(&words[i..], today) {
            found.push((i, i + n, a, b));
            i += n;
        } else if let Some((n, d)) = explicit(&words[i..], &orig[i..], &comma[i..], today) {
            found.push((i, i + n, d, d));
            i += n;
        } else {
            i += 1;
        }
    }
    if found.is_empty() {
        return None;
    }
    let since = found.iter().map(|f| f.2).min()?;
    let until = found.iter().map(|f| f.3).max()?;
    let said: Vec<String> = found.iter().map(|f| q[toks[f.0].0..toks[f.1 - 1].1].to_string()).collect();
    // The query without those words (and a bare "on", "in", "from" left before them is harmless: a stopword).
    let mut rest = String::new();
    let mut at = 0;
    for f in &found {
        rest.push_str(&q[at..toks[f.0].0]);
        at = toks[f.1 - 1].1;
    }
    rest.push_str(&q[at..]);
    Some((Said { since: since.to_string(), until: until.to_string(), words: said.join(", ") }, rest.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).collect::<Vec<_>>().join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn span(q: &str) -> Option<(String, String, String)> {
        in_query(q, d("2026-11-18")).map(|(s, rest)| (s.since, s.until, rest))
    }

    #[test]
    fn dates_in_english_ukrainian_and_russian_become_a_range() {
        let day = |x: &str, rest: &str| Some((x.to_string(), x.to_string(), rest.to_string()));
        assert_eq!(span("what did we decide on 12 November"), day("2026-11-12", "what did we decide on"));
        assert_eq!(span("gateway decision 2026-11-12"), day("2026-11-12", "gateway decision"));
        assert_eq!(span("November 12, 2025 budget"), day("2025-11-12", "budget"));
        assert_eq!(span("budget Nov 3rd"), day("2026-11-03", "budget"));
        assert_eq!(span("що вирішили 12 листопада"), day("2026-11-12", "що вирішили"));
        assert_eq!(span("что решили 12 ноября про бюджет"), day("2026-11-12", "что решили про бюджет"));
        assert_eq!(span("встреча 3-го октября"), day("2026-10-03", "встреча"));
        // A day still to come this year is last year's.
        assert_eq!(span("kickoff 20 December"), day("2025-12-20", "kickoff"));
        // Weeks: Monday to Sunday (18 November 2026 is a Wednesday).
        let week = Some(("2026-11-09".to_string(), "2026-11-15".to_string(), "status".to_string()));
        assert_eq!(span("status last week"), week);
        assert_eq!(span("статус минулого тижня"), week.clone().map(|w| (w.0, w.1, "статус".into())));
        assert_eq!(span("статус на прошлой неделе"), week.map(|w| (w.0, w.1, "статус".into())));
        assert_eq!(span("this week rollout"), Some(("2026-11-16".into(), "2026-11-18".into(), "rollout".into())));
        assert_eq!(span("вчера"), day("2026-11-17", ""));
        assert_eq!(span("last month invoices"), Some(("2026-10-01".into(), "2026-10-31".into(), "invoices".into())));
        // Two dates: from the first to the last.
        assert_eq!(span("between 3 November and 12 November"), Some(("2026-11-03".into(), "2026-11-12".into(), "between and".into())));
    }

    #[test]
    fn a_month_or_a_number_alone_is_not_a_date() {
        for q in ["the November release", "may we ship 12 stores", "Q4 2026 plan", "sprint 12", "31 February", "march on", "step 3 may fail", "march 5 teams", "the build 7 may break", "jan 12 rows"] {
            assert_eq!(span(q), None, "{q}");
        }
    }

    #[test]
    fn a_lowercase_english_month_is_a_date_only_in_a_dates_form() {
        let day = |x: &str, rest: &str| Some((x.to_string(), x.to_string(), rest.to_string()));
        assert_eq!(span("step 3 May fail"), day("2026-05-03", "step fail"), "capitalised: a date");
        assert_eq!(span("review 3 may 2026"), day("2026-05-03", "review"), "with a year");
        assert_eq!(span("may 3, the review"), day("2026-05-03", "the review"), "a day and a comma");
        assert_eq!(span("review 2026-05-03"), day("2026-05-03", "review"));
        assert_eq!(span("3 травня"), day("2026-05-03", ""), "Ukrainian and Russian month words are never other words");
    }
}
