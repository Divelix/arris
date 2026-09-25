//! `step::part21::parse` on any text: no panic, the same answer twice,
//! and an error that names a place inside the text. The reader
//! (`step::read`) takes the parser's place once it exists
//! (plans/step-reader step 17).
#![no_main]

use arris_io::step::part21;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Part 21 is ASCII; a byte that is not UTF-8 becomes U+FFFD, which
    // the parser must refuse as a token like any other stray character.
    let text = String::from_utf8_lossy(data);
    let first = part21::parse(&text);
    let second = part21::parse(&text);
    assert_eq!(first, second, "two parses of one text differ");
    if let Err(e) = &first {
        // Lines and columns count from 1; the last line may be the one
        // after a final newline, where an unexpected end is found.
        let lines = text.split('\n').count();
        assert!(e.line >= 1 && e.column >= 1, "{e}: not counted from 1");
        assert!(e.line as usize <= lines, "{e}: past the text's {lines} lines");
        if let Some(line) = text.split('\n').nth(e.line as usize - 1) {
            let width = line.chars().count() + 1;
            assert!(e.column as usize <= width, "{e}: past the line's {width} columns");
        }
    }
});
