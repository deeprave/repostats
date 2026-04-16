use unicode_segmentation::UnicodeSegmentation;

pub fn title_case(s: &str) -> String {
    s.split_word_bounds()
        .map(|w| {
            let mut g = w.graphemes(true);
            match g.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), g.as_str().to_lowercase()),
                None => String::new(),
            }
        })
        .collect()
}
