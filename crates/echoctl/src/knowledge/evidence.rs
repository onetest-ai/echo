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
    fn pdf_is_not_a_text_source() {
        for ok in ["a.md", "b.TXT", "c/d.csv", "e.json"] {
            assert!(text_source(Path::new(ok)), "{ok}");
        }
        for no in ["deck.pdf", "slides.pptx", "notes", "x.md.bak", "img.png"] {
            assert!(!text_source(Path::new(no)), "{no}");
        }
    }
}
