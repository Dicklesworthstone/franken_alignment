use super::*;

fn pieces(mode: PreTokenization, source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let length = mode.piece_length(&source[at..]);
        assert!(length != 0, "a complete grammar must consume {source:?} at {at}");
        result.push(&source[at..at + length]);
        at += length;
    }
    result
}

#[test]
fn unicode_gpt2_matches_independently_written_unicode_and_whitespace_splits() {
    for (source, expected) in [
        ("hello world!", vec!["hello", " world", "!"]),
        ("we're I'M we'll", vec!["we", "'re", " I", "'", "M", " we", "'ll"]),
        ("a12b", vec!["a", "12", "b"]),
        ("  a\t \r\nb  ", vec![" ", " a", "\t \r", "\n", "b", "  "]),
        ("a\u{0345}b", vec!["a", "\u{0345}", "b"]),
        ("é中𐐀", vec!["é中𐐀"]),
        ("１２Ⅲ²٤", vec!["１２Ⅲ²٤"]),
        ("a\u{a0}b", vec!["a", "\u{a0}", "b"]),
        ("a\u{200b}b", vec!["a", "\u{200b}", "b"]),
        ("'ſa", vec!["'", "ſa"]),
        ("foo—bar", vec!["foo", "—", "bar"]),
        ("🙂hello", vec!["🙂", "hello"]),
        ("!!hello", vec!["!!", "hello"]),
        ("!?\r\nx", vec!["!?", "\r", "\n", "x"]),
        (" \u{2003}x", vec![" ", "\u{2003}", "x"]),
        (" \r \nx \t", vec![" \r ", "\n", "x", " \t"]),
        ("\r\n", vec!["\r\n"]),
        ("１２３４５６７", vec!["１２３４５６７"]),
        ("\u{85}a", vec!["\u{85}", "a"]),
        ("a\0b", vec!["a", "\0", "b"]),
    ] {
        assert_eq!(pieces(PreTokenization::Gpt2Unicode17, source), expected, "{source:?}");
    }
}

#[test]
fn unicode_llama3_respects_simple_case_folding_scalar_groups_and_greedy_newlines() {
    for (source, expected) in [
        ("hello world!", vec!["hello", " world", "!"]),
        ("we're I'M we'll", vec!["we", "'re", " I", "'M", " we", "'ll"]),
        ("a12b", vec!["a", "12", "b"]),
        ("  a\t \r\nb  ", vec![" ", " a", "\t \r\n", "b", "  "]),
        ("a\u{0345}b", vec!["a", "\u{0345}b"]),
        ("é中𐐀", vec!["é中𐐀"]),
        ("１２Ⅲ²٤", vec!["１２Ⅲ", "²٤"]),
        ("a\u{a0}b", vec!["a", "\u{a0}b"]),
        ("a\u{200b}b", vec!["a", "\u{200b}b"]),
        ("'ſa", vec!["'ſ", "a"]),
        ("foo—bar", vec!["foo", "—bar"]),
        ("🙂hello", vec!["🙂hello"]),
        ("!!hello", vec!["!!", "hello"]),
        ("!?\r\nx", vec!["!?\r\n", "x"]),
        (" \u{2003}x", vec![" ", "\u{2003}x"]),
        (" \r \nx \t", vec![" \r \n", "x", " \t"]),
        ("\r\n", vec!["\r\n"]),
        ("１２３４５６７", vec!["１２３", "４５６", "７"]),
        ("\u{85}a", vec!["\u{85}a"]),
        ("a\0b", vec!["a", "\0b"]),
    ] {
        assert_eq!(pieces(PreTokenization::Llama3Unicode17, source), expected, "{source:?}");
    }
}

#[test]
fn unicode_tables_distinguish_general_categories_from_derived_alphabetic() {
    assert!(unicode17::is_letter('é'));
    assert!(unicode17::is_letter('中'));
    assert!(unicode17::is_letter('𐐀'));
    // Greek ypogegrammeni is Alphabetic but General_Category Mn, not Letter.
    assert!(!unicode17::is_letter('\u{0345}'));
    assert!(!unicode17::is_number('\u{0345}'));
    // Roman numerals are Letter_Number, and superscripts are Other_Number.
    for c in ['Ⅲ', '²', '٤', '４'] {
        assert!(unicode17::is_number(c));
        assert!(!unicode17::is_letter(c));
    }
    for c in ['\u{a0}', '\u{85}', '\u{2003}', '\u{2028}'] {
        assert!(unicode17::is_white_space(c));
    }
    for c in ['\u{200b}', '\u{1c}', '\0', '\u{180e}'] {
        assert!(!unicode17::is_white_space(c));
    }
    assert_eq!(unicode17::fold_simple('ſ'), 's');
    assert_eq!(unicode17::fold_simple('S'), 's');
    assert_eq!(PreTokenization::Gpt2Unicode17.unicode_version(), Some((17, 0, 0)));
    assert_eq!(PreTokenization::RawBytes.unicode_version(), None);
}

#[test]
fn unicode_short_mixed_inputs_always_partition_without_byte_rewriting() {
    let alphabet = ['a', 'Ⅲ', '²', ' ', '\r', '\n', '\u{0345}', '🙂', 'ſ', '\''];
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        for length in 0..=4_u32 {
            for mut code in 0..alphabet.len().pow(length) {
                let mut input = String::new();
                for _ in 0..length { input.push(alphabet[code % alphabet.len()]); code /= alphabet.len(); }
                let actual = pieces(mode, &input);
                assert!(actual.iter().all(|piece| !piece.is_empty()));
                assert_eq!(actual.concat(), input);
            }
        }
    }
}
