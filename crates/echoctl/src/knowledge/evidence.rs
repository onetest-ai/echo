//! "Came from" evidence (spec D35, D43): does a quote really stand in the text it is said to come from.

use std::path::Path;

/// Words a quote must have, so that "the" or "yes" can't vouch for anything.
const MIN_WORDS: usize = 3;

/// Is `quote` in `text`: both normalised as the work ledger normalises (`ledger::norm`: case, punctuation and spacing
/// don't matter), the quote at least three words, and found as one contiguous run of whole words.
pub fn found(text: &str, quote: &str) -> bool {
    let q = crate::ledger::norm(quote);
    if q.split(' ').filter(|w| !w.is_empty()).count() < MIN_WORDS {
        return false;
    }
    format!(" {} ", crate::ledger::norm(text)).contains(&format!(" {q} "))
}

/// The line (1-based) where `quote` starts in `text`, found as `found` finds it; of several places, the one nearest line
/// `near` (else the first). None when it isn't there.
pub fn line_of(text: &str, quote: &str, near: Option<usize>) -> Option<usize> {
    if !found(text, quote) {
        return None;
    }
    let q = format!(" {} ", crate::ledger::norm(quote));
    // The text normalised line by line, the space before each line's first word noted: `norm` makes any break between
    // words one space, a line's end too, so this is `found`'s text.
    let (mut all, mut starts) = (String::from(" "), vec![]);
    for (i, l) in text.lines().enumerate() {
        let n = crate::ledger::norm(l);
        if !n.is_empty() {
            starts.push((all.len() - 1, i + 1));
            all.push_str(&n);
            all.push(' ');
        }
    }
    let mut lines = vec![];
    let mut from = 0;
    while let Some(i) = all[from..].find(&q) {
        let at = from + i;
        lines.extend(starts.iter().rev().find(|(s, _)| *s <= at).map(|(_, l)| *l));
        from = at + 1;
    }
    match near {
        Some(n) => lines.into_iter().min_by_key(|l| l.abs_diff(n)),
        None => lines.first().copied(),
    }
}

/// A file whose text Echo can check a quote against: `.md .txt .csv .json` (a PDF or a deck is not one).
pub fn text_source(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "txt" | "csv" | "json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_must_be_one_span_of_three_words() {
        let text = "The team agreed: \"move the pilot to 18 November\", and keep the pricing page.";
        assert!(found(text, "Move the pilot to 18 november"));
        assert!(found(text, "  the pricing   PAGE "), "case and spacing don't matter");
        assert!(found(text, "keep the pricing"));
        assert!(!found(text, "pilot to"), "two words are not enough");
        assert!(!found(text, "move the 18 November"), "words skipped over: not one span");
        assert!(!found(text, "the pilot to 18 Nov"), "a part of a word is not a word");
        assert!(!found(text, ""));
        assert!(!found("", "move the pilot"));
    }

    #[test]
    fn quotes_in_other_scripts_punctuation_and_lines() {
        let text = "Рішення: ПЕРЕНЕСТИ\n  Пілот   на 18 Листопада, далі — без змін.";
        assert!(found(text, "перенести пілот на 18 листопада"), "Cyrillic, mixed case, other spacing, across a line");
        assert!(!found(text, "перенести пілот на 19 листопада"));
        assert!(!found(text, "пілот на"), "fewer than three words");
        let t = "He said, \"we'll ship it -- on Friday!\" (probably).";
        assert!(found(t, "\"We'll ship it, on Friday\""), "punctuation around and inside the quote");
        assert!(found(t, "ship it on friday"));
        assert!(found("first line ends here\nsecond line starts", "ends here second line"), "a quote may cross a newline");
        assert!(!found(t, "ship, it"), "two words after punctuation is still two words");
    }

    #[test]
    fn a_quote_is_on_the_line_it_starts_on() {
        let text = "---\ntitle: Northwind Retail\n---\n\n## Roadmap\n- Phase 2 go-live: **12 December**,\n  before the holiday freeze.\n\nThe go-live moves later.\n\nThe go-live moves later.\n";
        assert_eq!(line_of(text, "Phase 2 go-live: 12 December, before the holiday freeze", None), Some(6), "across a line break and Markdown");
        assert_eq!(line_of(text, "the go-live moves later", None), Some(9), "of several: the first");
        assert_eq!(line_of(text, "the go-live moves later", Some(12)), Some(11), "of several: the one nearest the line given");
        assert_eq!(line_of(text, "title Northwind Retail", None), Some(2), "properties' lines count");
        assert_eq!(line_of(text, "Phase 3 go-live", None), None);
        assert_eq!(line_of(text, "Roadmap", None), None, "fewer than three words, as `found`");
    }

    #[test]
    fn pdf_is_not_a_text_source() {
        for ok in ["a.md", "b.TXT", "c/d.csv", "e.json"] {
            assert!(text_source(Path::new(ok)), "{ok}");
        }
        for no in ["deck.pdf", "slides.pptx", "notes", "x.md.bak", "img.png"] {
            assert!(!text_source(Path::new(no)), "{no}");
        }
    }
}
