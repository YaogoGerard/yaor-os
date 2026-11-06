// Police bitmap 5x7 incorporée (étape 2) — extraite de `yaor-panel` pour
// être partagée avec `yaor-dock`.
//
// Convention « un besoin = un fournisseur » : pas de bibliothèque de texte
// ni de police système dans l'image live, les capitales et les chiffres
// utiles aux libellés du bureau sont dessinés littéralement (7 lignes de
// 5 colonnes, « X » = pixel allumé). Ajouter un glyphe = ajouter une
// entrée ; un caractère absent avance le curseur sans rien dessiner.

use crate::raster::put;

/// Glyphes 5x7 : 7 lignes de 5 colonnes, « X » = pixel allumé.
pub const GLYPHS: &[(char, [&str; 7])] = &[
    (
        '0',
        ["0XXX0", "0..X0", "X..X0", "X..X0", "X..X0", "X..X0", "0XXX0"],
    ),
    (
        '1',
        ["..X..", ".XX..", "..X..", "..X..", "..X..", "..X..", "0XXXX"],
    ),
    (
        '2',
        ["0XXX0", "X...X", "....X", "...X.", "..X..", ".X...", "XXXXX"],
    ),
    (
        '3',
        ["XXXXX", "....X", "...X.", "..XX.", "....X", "....X", "XXXXX"],
    ),
    (
        '4',
        ["X...X", "X...X", "X...X", "XXXXX", "....X", "....X", "....X"],
    ),
    (
        '5',
        ["XXXXX", "X....", "XXXX.", "....X", "....X", "X...X", "0XXX0"],
    ),
    (
        '6',
        [".XXX.", "X....", "X....", "XXXX.", "X...X", "X...X", ".XXX."],
    ),
    (
        '7',
        ["XXXXX", "....X", "...X.", "..X..", ".X...", ".X...", ".X..."],
    ),
    (
        '8',
        [".XXX.", "X...X", "X...X", ".XXX.", "X...X", "X...X", ".XXX."],
    ),
    (
        '9',
        [".XXX.", "X...X", "X...X", ".XXXX", "....X", "....X", ".XXX."],
    ),
    (
        'A',
        [".XXX.", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    ),
    (
        'E',
        ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "XXXXX"],
    ),
    (
        'M',
        ["X...X", "XX.XX", "X.X.X", "X.X.X", "X...X", "X...X", "X...X"],
    ),
    (
        'N',
        ["X...X", "XX..X", "XX..X", "X.X.X", "X..XX", "X..XX", "X...X"],
    ),
    (
        'O',
        [".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    ),
    (
        'R',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X.X..", "X..X.", "X...X"],
    ),
    (
        'S',
        [".XXXX", "X....", "X....", ".XXX.", "....X", "....X", "XXXX."],
    ),
    (
        'T',
        ["XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "..X.."],
    ),
    (
        'U',
        ["X...X", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    ),
    (
        'W',
        ["X...X", "X...X", "X...X", "X.X.X", "X.X.X", "X.X.X", ".X.X."],
    ),
    (
        'X',
        ["X...X", "X...X", ".X.X.", "..X..", "..X..", ".X.X.", "X...X"],
    ),
    (
        'Y',
        ["X...X", "X...X", ".X.X.", "..X..", "..X..", "..X..", "..X.."],
    ),
    (
        '.',
        [".....", ".....", ".....", ".....", ".....", ".XX..", ".XX.."],
    ),
    (
        ':',
        [".....", ".XX..", ".XX..", ".....", ".....", ".XX..", ".XX.."],
    ),
    (
        'C',
        [".XXX.", "X...X", "X....", "X....", "X....", "X...X", ".XXX."],
    ),
    (
        'D',
        ["XXXX.", "X...X", "X...X", "X...X", "X...X", "X...X", "XXXX."],
    ),
    (
        'F',
        ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "X...."],
    ),
    (
        'I',
        ["XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "XXXXX"],
    ),
    (
        'J',
        ["....X", "....X", "....X", "....X", "X...X", "X...X", ".XXX."],
    ),
    (
        'L',
        ["X....", "X....", "X....", "X....", "X....", "X....", "XXXXX"],
    ),
    (
        'V',
        ["X...X", "X...X", "X...X", "X...X", ".X.X.", ".X.X.", "..X.."],
    ),
    // 1d.menu : glyphes du volet (libellés + symboles de tuiles).
    (
        'B',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X...X", "X...X", "XXXX."],
    ),
    (
        'G',
        ["0XXX.", "X...X", "X....", "X..XX", "X...X", "X...X", "0XXX0"],
    ),
    (
        'H',
        ["X...X", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    ),
    (
        'P',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X....", "X....", "X...."],
    ),
    (
        'Q',
        ["0XXX0", "X...X", "X...X", "X...X", "X.X.X", "X..X.", "0XX.X"],
    ),
    (
        '>',
        ["X....", ".X...", "..X..", "...X.", "..X..", ".X...", "X...."],
    ),
];

/// Glyphe d'un caractère (insensible à la casse) — `None` si absent.
pub fn glyph(ch: char) -> Option<&'static [&'static str; 7]> {
    let up = ch.to_ascii_uppercase();
    GLYPHS.iter().find(|(c, _)| *c == up).map(|(_, g)| g)
}

/// Largeur d'un texte rendu (6 px par glyphe : 5 + 1 d'espacement).
pub fn text_width(s: &str) -> i32 {
    (s.chars().count() as i32) * 6
}

/// Texte 5x7 à l'échelle 1.
pub fn draw_text(canvas: &mut [u8], w: i32, h: i32, text: &str, x0: i32, y0: i32, color: u32) {
    draw_text_scaled(canvas, w, h, text, x0, y0, color, 1);
}

/// Texte 5x7 agrandi d'un facteur `sc` (entêtes et glyphes de tuiles).
pub fn draw_text_scaled(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    text: &str,
    x0: i32,
    y0: i32,
    color: u32,
    sc: i32,
) {
    let sc = sc.max(1);
    let mut x = x0;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for (rx, c) in row.chars().enumerate() {
                    if c == 'X' {
                        for sy in 0..sc {
                            for sx in 0..sc {
                                put(
                                    canvas,
                                    w,
                                    h,
                                    x + rx as i32 * sc + sx,
                                    y0 + ry as i32 * sc + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
        }
        x += 6 * sc;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphe_insensible_a_la_casse() {
        assert_eq!(glyph('a'), glyph('A'));
        assert!(glyph('A').is_some());
        assert!(glyph(' ').is_none(), "espace absent : curseur seul");
    }

    #[test]
    fn largeur_par_glyphe() {
        assert_eq!(text_width(""), 0);
        assert_eq!(text_width("YAOR"), 24);
        assert_eq!(text_width("A.B"), 18);
    }

    #[test]
    fn draw_text_allume_les_bons_pixels() {
        // « I » : la première ligne est pleine (XXXXX).
        let mut canvas = vec![0u8; 8 * 8 * 4];
        draw_text(&mut canvas, 8, 8, "I", 1, 1, 0xFF_FF_FF_FF);
        let off = ((1 * 8) + 3) * 4; // 3e colonne de la ligne 1
        assert_eq!(canvas[off + 3], 0xFF, "pixel du glyphe allumé");
        let off_vide = ((4 * 8) + 1) * 4; // sous la barre supérieure du « I »
        assert_eq!(canvas[off_vide + 3], 0);
    }
}
