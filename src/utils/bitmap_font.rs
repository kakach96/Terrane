//! Built-in 5x7 bitmap font for label (TextSymbolizer) rendering.
//!
//! Zero-dependency ASCII glyph set so SLD labels render without bundling a
//! TTF/OTF font. Each glyph is 7 rows of 5 columns; a row is a 5-char string
//! where `X` marks an on pixel and `.` an off pixel.

/// A single 5x7 glyph (7 rows of 5 columns).
pub struct Glyph {
    rows: [&'static str; 7],
}

impl Glyph {
    /// Whether the pixel at (col, row) is set. `col` in 0..5, `row` in 0..7.
    #[inline]
    pub fn pixel(&self, col: u32, row: u32) -> bool {
        self.rows
            .get(row as usize)
            .and_then(|r| r.chars().nth(col as usize))
            .map(|c| c == 'X')
            .unwrap_or(false)
    }
}

const GLYPHS: &[(char, Glyph)] = &[
    (
        ' ',
        Glyph {
            rows: [
                ".....", ".....", ".....", ".....", ".....", ".....", ".....",
            ],
        },
    ),
    (
        '!',
        Glyph {
            rows: [
                "..X..", "..X..", "..X..", "..X..", "..X..", ".....", "..X..",
            ],
        },
    ),
    (
        '"',
        Glyph {
            rows: [
                ".X.X.", ".X.X.", ".X.X.", ".....", ".....", ".....", ".....",
            ],
        },
    ),
    (
        '#',
        Glyph {
            rows: [
                ".X.X.", ".X.X.", "XXXXX", ".X.X.", "XXXXX", ".X.X.", ".X.X.",
            ],
        },
    ),
    (
        '$',
        Glyph {
            rows: [
                "..X..", ".XXXX", "X.X..", ".XXX.", "..X.X", "XXXX.", "..X..",
            ],
        },
    ),
    (
        '%',
        Glyph {
            rows: [
                "XX..X", "XX.X.", "...X.", "..X..", ".X...", ".X.XX", "X..XX",
            ],
        },
    ),
    (
        '&',
        Glyph {
            rows: [
                ".XX..", "X..X.", "X.X..", ".XX..", "X.X.X", "X..X.", ".XX.X",
            ],
        },
    ),
    (
        '\'',
        Glyph {
            rows: [
                "..X..", "..X..", "..X..", ".....", ".....", ".....", ".....",
            ],
        },
    ),
    (
        '(',
        Glyph {
            rows: [
                "...X.", "..X..", ".X...", ".X...", ".X...", "..X..", "...X.",
            ],
        },
    ),
    (
        ')',
        Glyph {
            rows: [
                ".X...", "..X..", "...X.", "...X.", "...X.", "..X..", ".X...",
            ],
        },
    ),
    (
        '*',
        Glyph {
            rows: [
                ".....", ".X.X.", ".XXX.", "XXXXX", ".XXX.", ".X.X.", ".....",
            ],
        },
    ),
    (
        '+',
        Glyph {
            rows: [
                ".....", "..X..", "..X..", "XXXXX", "..X..", "..X..", ".....",
            ],
        },
    ),
    (
        ',',
        Glyph {
            rows: [
                ".....", ".....", ".....", ".....", "..X..", "..X..", ".X...",
            ],
        },
    ),
    (
        '-',
        Glyph {
            rows: [
                ".....", ".....", ".....", "XXXXX", ".....", ".....", ".....",
            ],
        },
    ),
    (
        '.',
        Glyph {
            rows: [
                ".....", ".....", ".....", ".....", ".....", "..X..", "..X..",
            ],
        },
    ),
    (
        '/',
        Glyph {
            rows: [
                "....X", "....X", "...X.", "..X..", ".X...", "X....", "X....",
            ],
        },
    ),
    (
        '0',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X..XX", "X.X.X", "XX..X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        '1',
        Glyph {
            rows: [
                "..X..", ".XX..", "..X..", "..X..", "..X..", "..X..", ".XXX.",
            ],
        },
    ),
    (
        '2',
        Glyph {
            rows: [
                ".XXX.", "X...X", "....X", "...X.", "..X..", ".X...", "XXXXX",
            ],
        },
    ),
    (
        '3',
        Glyph {
            rows: [
                "XXXXX", "....X", "...X.", "..XX.", "....X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        '4',
        Glyph {
            rows: [
                "...X.", "..XX.", ".X.X.", "X..X.", "XXXXX", "...X.", "...X.",
            ],
        },
    ),
    (
        '5',
        Glyph {
            rows: [
                "XXXXX", "X....", "XXXX.", "....X", "....X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        '6',
        Glyph {
            rows: [
                "..XX.", ".X...", "X....", "XXXX.", "X...X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        '7',
        Glyph {
            rows: [
                "XXXXX", "....X", "...X.", "..X..", ".X...", ".X...", ".X...",
            ],
        },
    ),
    (
        '8',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X...X", ".XXX.", "X...X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        '9',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X...X", ".XXXX", "....X", "...X.", ".XX..",
            ],
        },
    ),
    (
        ':',
        Glyph {
            rows: [
                ".....", "..X..", "..X..", ".....", "..X..", "..X..", ".....",
            ],
        },
    ),
    (
        ';',
        Glyph {
            rows: [
                ".....", "..X..", "..X..", ".....", "..X..", "..X..", ".X...",
            ],
        },
    ),
    (
        '<',
        Glyph {
            rows: [
                "...X.", "..X..", ".X...", "X....", ".X...", "..X..", "...X.",
            ],
        },
    ),
    (
        '=',
        Glyph {
            rows: [
                ".....", ".....", "XXXXX", ".....", "XXXXX", ".....", ".....",
            ],
        },
    ),
    (
        '>',
        Glyph {
            rows: [
                ".X...", "..X..", "...X.", "....X", "...X.", "..X..", ".X...",
            ],
        },
    ),
    (
        '?',
        Glyph {
            rows: [
                ".XXX.", "X...X", "....X", "...X.", "..X..", ".....", "..X..",
            ],
        },
    ),
    (
        '@',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X.XXX", "X.X.X", "X.XXX", "X....", ".XXX.",
            ],
        },
    ),
    (
        'A',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X",
            ],
        },
    ),
    (
        'B',
        Glyph {
            rows: [
                "XXXX.", "X...X", "X...X", "XXXX.", "X...X", "X...X", "XXXX.",
            ],
        },
    ),
    (
        'C',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X....", "X....", "X....", "X...X", ".XXX.",
            ],
        },
    ),
    (
        'D',
        Glyph {
            rows: [
                "XXXX.", "X...X", "X...X", "X...X", "X...X", "X...X", "XXXX.",
            ],
        },
    ),
    (
        'E',
        Glyph {
            rows: [
                "XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "XXXXX",
            ],
        },
    ),
    (
        'F',
        Glyph {
            rows: [
                "XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "X....",
            ],
        },
    ),
    (
        'G',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X....", "X.XXX", "X...X", "X...X", ".XXXX",
            ],
        },
    ),
    (
        'H',
        Glyph {
            rows: [
                "X...X", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X",
            ],
        },
    ),
    (
        'I',
        Glyph {
            rows: [
                ".XXX.", "..X..", "..X..", "..X..", "..X..", "..X..", ".XXX.",
            ],
        },
    ),
    (
        'J',
        Glyph {
            rows: [
                "..XXX", "...X.", "...X.", "...X.", "...X.", "X..X.", ".XX..",
            ],
        },
    ),
    (
        'K',
        Glyph {
            rows: [
                "X...X", "X..X.", "X.X..", "XX...", "X.X..", "X..X.", "X...X",
            ],
        },
    ),
    (
        'L',
        Glyph {
            rows: [
                "X....", "X....", "X....", "X....", "X....", "X....", "XXXXX",
            ],
        },
    ),
    (
        'M',
        Glyph {
            rows: [
                "X...X", "XX.XX", "X.X.X", "X.X.X", "X...X", "X...X", "X...X",
            ],
        },
    ),
    (
        'N',
        Glyph {
            rows: [
                "X...X", "XX..X", "X.X.X", "X..XX", "X...X", "X...X", "X...X",
            ],
        },
    ),
    (
        'O',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        'P',
        Glyph {
            rows: [
                "XXXX.", "X...X", "X...X", "XXXX.", "X....", "X....", "X....",
            ],
        },
    ),
    (
        'Q',
        Glyph {
            rows: [
                ".XXX.", "X...X", "X...X", "X...X", "X.X.X", "X..X.", ".XX.X",
            ],
        },
    ),
    (
        'R',
        Glyph {
            rows: [
                "XXXX.", "X...X", "X...X", "XXXX.", "X.X..", "X..X.", "X...X",
            ],
        },
    ),
    (
        'S',
        Glyph {
            rows: [
                ".XXXX", "X....", "X....", ".XXX.", "....X", "....X", "XXXX.",
            ],
        },
    ),
    (
        'T',
        Glyph {
            rows: [
                "XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "..X..",
            ],
        },
    ),
    (
        'U',
        Glyph {
            rows: [
                "X...X", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        'V',
        Glyph {
            rows: [
                "X...X", "X...X", "X...X", "X...X", "X...X", ".X.X.", "..X..",
            ],
        },
    ),
    (
        'W',
        Glyph {
            rows: [
                "X...X", "X...X", "X...X", "X.X.X", "X.X.X", "XX.XX", "X...X",
            ],
        },
    ),
    (
        'X',
        Glyph {
            rows: [
                "X...X", "X...X", ".X.X.", "..X..", ".X.X.", "X...X", "X...X",
            ],
        },
    ),
    (
        'Y',
        Glyph {
            rows: [
                "X...X", "X...X", ".X.X.", "..X..", "..X..", "..X..", "..X..",
            ],
        },
    ),
    (
        'Z',
        Glyph {
            rows: [
                "XXXXX", "....X", "...X.", "..X..", ".X...", "X....", "XXXXX",
            ],
        },
    ),
    (
        '[',
        Glyph {
            rows: [
                ".XXX.", ".X...", ".X...", ".X...", ".X...", ".X...", ".XXX.",
            ],
        },
    ),
    (
        '\\',
        Glyph {
            rows: [
                "X....", "X....", ".X...", "..X..", "...X.", "....X", "....X",
            ],
        },
    ),
    (
        ']',
        Glyph {
            rows: [
                ".XXX.", "...X.", "...X.", "...X.", "...X.", "...X.", ".XXX.",
            ],
        },
    ),
    (
        '^',
        Glyph {
            rows: [
                "..X..", ".X.X.", "X...X", ".....", ".....", ".....", ".....",
            ],
        },
    ),
    (
        '_',
        Glyph {
            rows: [
                ".....", ".....", ".....", ".....", ".....", ".....", "XXXXX",
            ],
        },
    ),
    (
        '`',
        Glyph {
            rows: [
                ".X...", "..X..", ".....", ".....", ".....", ".....", ".....",
            ],
        },
    ),
    (
        'a',
        Glyph {
            rows: [
                ".....", ".....", ".XXX.", "....X", ".XXXX", "X...X", ".XXXX",
            ],
        },
    ),
    (
        'b',
        Glyph {
            rows: [
                "X....", "X....", "XXXX.", "X...X", "X...X", "X...X", "XXXX.",
            ],
        },
    ),
    (
        'c',
        Glyph {
            rows: [
                ".....", ".....", ".XXX.", "X....", "X....", "X...X", ".XXX.",
            ],
        },
    ),
    (
        'd',
        Glyph {
            rows: [
                "....X", "....X", ".XXXX", "X...X", "X...X", "X...X", ".XXXX",
            ],
        },
    ),
    (
        'e',
        Glyph {
            rows: [
                ".....", ".....", ".XXX.", "X...X", "XXXXX", "X....", ".XXX.",
            ],
        },
    ),
    (
        'f',
        Glyph {
            rows: [
                "..XX.", ".X..X", ".X...", "XXX..", ".X...", ".X...", ".X...",
            ],
        },
    ),
    (
        'g',
        Glyph {
            rows: [
                ".....", ".XXXX", "X...X", "X...X", ".XXXX", "....X", ".XXX.",
            ],
        },
    ),
    (
        'h',
        Glyph {
            rows: [
                "X....", "X....", "XXXX.", "X...X", "X...X", "X...X", "X...X",
            ],
        },
    ),
    (
        'i',
        Glyph {
            rows: [
                "..X..", ".....", ".XX..", "..X..", "..X..", "..X..", ".XXX.",
            ],
        },
    ),
    (
        'j',
        Glyph {
            rows: [
                "...X.", ".....", "..XX.", "...X.", "...X.", "...X.", ".XX..",
            ],
        },
    ),
    (
        'k',
        Glyph {
            rows: [
                "X....", "X....", "X..X.", "X.X..", "XX...", "X.X..", "X..X.",
            ],
        },
    ),
    (
        'l',
        Glyph {
            rows: [
                ".XX..", "..X..", "..X..", "..X..", "..X..", "..X..", ".XXX.",
            ],
        },
    ),
    (
        'm',
        Glyph {
            rows: [
                ".....", ".....", "XX.X.", "X.X.X", "X.X.X", "X...X", "X...X",
            ],
        },
    ),
    (
        'n',
        Glyph {
            rows: [
                ".....", ".....", "XXXX.", "X...X", "X...X", "X...X", "X...X",
            ],
        },
    ),
    (
        'o',
        Glyph {
            rows: [
                ".....", ".....", ".XXX.", "X...X", "X...X", "X...X", ".XXX.",
            ],
        },
    ),
    (
        'p',
        Glyph {
            rows: [
                ".....", "XXXX.", "X...X", "X...X", "XXXX.", "X....", "X....",
            ],
        },
    ),
    (
        'q',
        Glyph {
            rows: [
                ".....", ".XXXX", "X...X", "X...X", ".XXXX", "....X", "....X",
            ],
        },
    ),
    (
        'r',
        Glyph {
            rows: [
                ".....", ".....", "X.XX.", "XX..X", "X....", "X....", "X....",
            ],
        },
    ),
    (
        's',
        Glyph {
            rows: [
                ".....", ".....", ".XXXX", "X....", ".XXX.", "....X", "XXXX.",
            ],
        },
    ),
    (
        't',
        Glyph {
            rows: [
                ".X...", ".X...", "XXX..", ".X...", ".X...", ".X..X", "..XX.",
            ],
        },
    ),
    (
        'u',
        Glyph {
            rows: [
                ".....", ".....", "X...X", "X...X", "X...X", "X..XX", ".XX.X",
            ],
        },
    ),
    (
        'v',
        Glyph {
            rows: [
                ".....", ".....", "X...X", "X...X", "X...X", ".X.X.", "..X..",
            ],
        },
    ),
    (
        'w',
        Glyph {
            rows: [
                ".....", ".....", "X...X", "X...X", "X.X.X", "X.X.X", ".X.X.",
            ],
        },
    ),
    (
        'x',
        Glyph {
            rows: [
                ".....", ".....", "X...X", ".X.X.", "..X..", ".X.X.", "X...X",
            ],
        },
    ),
    (
        'y',
        Glyph {
            rows: [
                ".....", "X...X", "X...X", "X...X", ".XXXX", "....X", ".XXX.",
            ],
        },
    ),
    (
        'z',
        Glyph {
            rows: [
                ".....", ".....", "XXXXX", "...X.", "..X..", ".X...", "XXXXX",
            ],
        },
    ),
    (
        '{',
        Glyph {
            rows: [
                "...XX", "..X..", "..X..", ".X...", "..X..", "..X..", "...XX",
            ],
        },
    ),
    (
        '|',
        Glyph {
            rows: [
                "..X..", "..X..", "..X..", "..X..", "..X..", "..X..", "..X..",
            ],
        },
    ),
    (
        '}',
        Glyph {
            rows: [
                "XX...", "..X..", "..X..", "...X.", "..X..", "..X..", "XX...",
            ],
        },
    ),
    (
        '~',
        Glyph {
            rows: [
                ".....", ".X..X", "X.XX.", ".....", ".....", ".....", ".....",
            ],
        },
    ),
];

/// Look up a glyph for a character (unknown characters render as a space).
pub fn glyph_for(c: char) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|(ch, _)| *ch == c).map(|(_, g)| g)
}

/// Width of a text string in pixels at the given scale
/// (glyph width 5px, advance 6px: 5 columns + 1 spacing).
pub fn text_width(text: &str, scale: f64) -> u32 {
    let chars = text.chars().count() as u32;
    if chars == 0 {
        return 0;
    }
    let glyph_w = (5.0 * scale).ceil() as u32;
    let advance = (6.0 * scale).ceil() as u32;
    glyph_w + (chars - 1) * advance
}

/// Height of rendered text in pixels (7 rows scaled).
pub fn text_height(scale: f64) -> u32 {
    (7.0 * scale).ceil() as u32
}

/// Draw a text string with the top-left corner at (x, y), scaling the glyphs
/// by `scale`. On-pixels are reported through `put(x, y)` — callers decide how
/// to composite (halos / opacity handled by the caller).
pub fn draw_text<F>(x: i32, y: i32, text: &str, scale: f64, mut put: F)
where
    F: FnMut(u32, u32),
{
    let gw = (5.0 * scale).ceil() as i32;
    let advance = gw + (scale.ceil() as i32).max(1);

    let mut cursor_x = x;

    for c in text.chars() {
        if let Some(g) = glyph_for(c) {
            for row in 0..7u32 {
                for col in 0..5u32 {
                    if !g.pixel(col, row) {
                        continue;
                    }
                    let px = cursor_x + (col as f64 * scale).floor() as i32;
                    let py = y + (row as f64 * scale).floor() as i32;
                    for dy in 0..((scale.ceil() as i32).max(1)) {
                        for dx in 0..((scale.ceil() as i32).max(1)) {
                            put((px + dx) as u32, (py + dy) as u32);
                        }
                    }
                }
            }
        }
        cursor_x += advance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_lookup() {
        assert!(glyph_for('A').is_some());
        assert!(glyph_for('z').is_some());
        assert!(glyph_for('0').is_some());
        assert!(glyph_for(' ').is_some());
        assert!(glyph_for('漢').is_none());
    }

    #[test]
    fn test_text_dimensions() {
        assert_eq!(text_width("A", 1.0), 5);
        assert_eq!(text_width("AB", 1.0), 11);
        assert_eq!(text_height(1.0), 7);
        assert_eq!(text_height(2.0), 14);
        assert_eq!(text_width("", 1.0), 0);
    }

    #[test]
    fn test_draw_text_writes_pixels() {
        let mut img = image::RgbaImage::new(32, 16);
        let mut count = 0u32;
        draw_text(2, 2, "A", 1.0, |x, y| {
            // Pixels must be in-bounds; nothing panicked.
            img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            count += 1;
        });
        // 'A' has 18 on-pixels at scale 1.
        assert_eq!(count, 18);
        assert_eq!(img.dimensions(), (32, 16));
    }
}
