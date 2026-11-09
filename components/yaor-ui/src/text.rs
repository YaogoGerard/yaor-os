// Police réelle du bureau YAOR (v0.6 redesign — passation, étape 1).
//
// Remplace la bitmap 5x7 (`font.rs`) : minuscules et accents pour les
// libellés conformes aux rendus de docs/05-ui-ux/design/ (« Fichiers »,
// « éditeur — ~/yaor/desktop-core », « Rechercher une application… »).
//
// « Un besoin = un fournisseur » : aucune bibliothèque de texte n'est
// vendue dans l'arbre Smithay (ADR-018), donc le moteur est ici — un
// **rastérisateur TrueType pur Rust** (head/maxp/hhea/hmtx/cmap/loca/
// glyf, contours composites résolus) au-dessus de deux faces embarquées
// à la compilation : Noto Sans Regular et Bold (licence OFL, copiées
// hors ligne depuis /usr/share/fonts/noto — aucun téléchargement au
// build).
//
// Contrat de rendu : canvas Argb8888 petit boutisme (octets B,G,R,A),
// y vers le bas, comme raster.rs. `draw_text` prend l'angle supérieur
// gauche de la ligne de texte ; la hauteur de ligne est `text_height`
// (metrics hhea de la face). Le glyphe est antialiasé (couverture
// 0..255, quatre sous-échantillons verticaux × recouvrement horizontal
// exact, remplissage non-nul suivant l'orientation des contours) puis
// mélangé sur le canvas.
//
// Coûts : la face est parseée une seule fois (OnceLock) ; chaque
// (caractère, taille, poids) est pixellisé une seule fois (cache
// global) — le rendu des frames ne fait que copier des couvertures.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Famille (fichier TTF) : les libellés du design utilisent deux poids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Bold,
}

/// Bitmap de glyphe antialiasé (couverture 0..255, une ligne = `w`
/// octets). `left`/`top` sont relatifs à la ligne de base du texte
/// (x vers la droite, y vers le bas) : le glyphe se pose à
/// (plume + left, baseline + top).
#[derive(Debug, Clone)]
pub struct GlyphBitmap {
    pub w: i32,
    pub h: i32,
    pub left: i32,
    pub top: i32,
    pub cov: Vec<u8>,
}

impl GlyphBitmap {
    /// True quand le glyphe ne dessine strictement rien (espace).
    pub fn is_empty(&self) -> bool {
        self.cov.iter().all(|&c| c == 0)
    }
}

// ---------------------------------------------------------------------------
// Faces TrueType embarquées + parse
// ---------------------------------------------------------------------------

const SANS_REGULAR: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");
const SANS_BOLD: &[u8] = include_bytes!("../assets/NotoSans-Bold.ttf");

/// Face parseée (données statiques, parse unique au premier usage).
struct Faces {
    reg: Face,
    bold: Face,
}

static FACES: OnceLock<Option<Faces>> = OnceLock::new();

/// Indexe les tables de la face une fois : Option falsifié si la face
/// n'est pas lisible — le bureau rend le texte tout juste rien
/// (dégradation douce) plutôt que de paniquer.
fn faces() -> &'static Option<Faces> {
    FACES.get_or_init(|| {
        Some(Faces {
            reg: Face::parse(SANS_REGULAR)?,
            bold: Face::parse(SANS_BOLD)?,
        })
    })
}

/// État du moteur : `false` déclenche les replis (largeur 0, rien
/// dessiné) — le bureau reste utilisable.
pub fn ready() -> bool {
    faces().is_some()
}

/// Face parseée (données statiques, table高峰 offset/len par nom).
struct Face {
    data: &'static [u8],
    units_per_em: u16,
    num_glyphs: u16,
    index_to_loc: i32,
    num_h_metrics: u16,
    ascender: i16,
    descender: i16,
    tables: HashMap<u32, (usize, usize)>,
}

fn rd_u16(d: &[u8], o: usize) -> u16 {
    if o + 1 < d.len() {
        u16::from_be_bytes([d[o], d[o + 1]])
    } else {
        0
    }
}
fn rd_i16(d: &[u8], o: usize) -> i16 {
    rd_u16(d, o) as i16
}
fn rd_u32(d: &[u8], o: usize) -> u32 {
    if o + 3 < d.len() {
        u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
    } else {
        0
    }
}
fn f2dot14(d: &[u8], o: usize) -> f64 {
    rd_i16(d, o) as f64 / 16_384.0
}
fn tag_u32(tag: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*tag)
}

fn word_tag(tag: &str) -> u32 {
    let mut name = [0u8; 4];
    for (i, c) in tag.bytes().enumerate().take(4) {
        name[i] = c;
    }
    u32::from_be_bytes(name)
}

impl Face {
    /// Parse l'entête SFNT et les tables utiles (head, maxp, hhea).
    fn parse(data: &'static [u8]) -> Option<Face> {
        if data.len() < 12 || rd_u32(data, 0) != 0x0001_0000 {
            return None;
        }
        let num_tables = rd_u16(data, 4) as usize;
        let mut tables = HashMap::new();
        for i in 0..num_tables {
            let rec = 12 + i * 16;
            if rec + 16 > data.len() {
                return None;
            }
            let tag = [data[rec], data[rec + 1], data[rec + 2], data[rec + 3]];
            let off = rd_u32(data, rec + 8) as usize;
            let len = rd_u32(data, rec + 12) as usize;
            tables.insert(u32::from_be_bytes(tag), (off, len));
        }
        let head_off = tables.get(&tag_u32(b"head"))?.0;
        let upem = rd_u16(data, head_off + 18);
        if upem == 0 {
            return None;
        }
        let index_to_loc = rd_i16(data, head_off + 50);
        let maxp_off = tables.get(&tag_u32(b"maxp"))?.0;
        let num_glyphs = rd_u16(data, maxp_off + 4);
        let hhea_off = tables.get(&tag_u32(b"hhea"))?.0;
        for req in ["glyf", "loca", "cmap", "hmtx"] {
            if !tables.contains_key(&word_tag(req)) {
                return None;
            }
        }
        Some(Face {
            tables,
            data,
            units_per_em: upem,
            num_glyphs,
            index_to_loc: index_to_loc as i32,
            num_h_metrics: rd_u16(data, hhea_off + 34),
            ascender: rd_i16(data, hhea_off + 4),
            descender: rd_i16(data, hhea_off + 6),
        })
    }

    fn table(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        let &(off, len) = self.tables.get(&tag_u32(tag))?;
        self.data.get(off..off + len)
    }

    fn scale(&self, size: u32) -> f64 {
        size as f64 / self.units_per_em as f64
    }

    /// Metrics verticaux à la taille `size` : (ascent, descent) px, y
    /// vers le bas (la descent si trouve SOUS la baseline). La hauteur
    /// de ligne est ascent + descent.
    fn v_metrics(&self, size: u32) -> (i32, i32) {
        let s = self.scale(size);
        (
            (self.ascender as f64 * s).ceil() as i32,
            (-(self.descender as f64) * s).ceil() as i32,
        )
    }

    /// Avancement d'un glyphe (px, arrondi). Repli : taille/2.
    fn advance(&self, gid: u16, size: u32) -> i32 {
        let s = self.scale(size);
        let hm = match self.table(b"hmtx") {
            Some(hm) => hm,
            None => return (size as i32 / 2).max(1),
        };
        let idx = gid.min(self.num_h_metrics.saturating_sub(1)) as usize;
        (rd_u16(hm, idx * 4) as f64 * s).round() as i32
    }

    /// Glyph ID d'un caractère (cmap format 4 BMP, repli format 12).
    fn glyph_id(&self, ch: char) -> u16 {
        let code = ch as u32;
        if code > 0xFFFF {
            return 0;
        }
        let cm = match self.table(b"cmap") {
            Some(c) => c,
            None => return 0,
        };
        if cm.len() < 4 {
            return 0;
        }
        let n_sub = rd_u16(cm, 2) as usize;
        for i in 0..n_sub {
            let rec = 4 + i * 8;
            if rec + 8 > cm.len() {
                break;
            }
            let (plat, enc) = (rd_u16(cm, rec), rd_u16(cm, rec + 2));
            let soff = rd_u32(cm, rec + 4) as usize;
            let windows = plat == 3 && (enc == 1 || enc == 0);
            if !(windows || plat == 0) {
                continue;
            }
            if soff < cm.len() {
                match rd_u16(cm, soff) {
                    4 => {
                        let g = Self::cmap4(cm, soff, code);
                        if g != 0 {
                            return g;
                        }
                    }
                    12 => {
                        if let Some(g) = Self::cmap12(cm, soff, code) {
                            return g;
                        }
                    }
                    _ => {}
                }
            }
        }
        0
    }

    fn cmap4(cm: &[u8], off: usize, code: u32) -> u16 {
        let seg_x2 = rd_u16(cm, off + 6) as usize;
        if seg_x2 == 0 {
            return 0;
        }
        let seg = seg_x2 / 2;
        let end = off + 14;
        let start = end + seg_x2 + 2;
        let delta = start + seg_x2;
        let range = delta + seg_x2;
        for i in 0..seg {
            let e = rd_u16(cm, end + i * 2) as u32;
            let s = rd_u16(cm, start + i * 2) as u32;
            if (s..=e).contains(&code) {
                let ro = rd_u16(cm, range + i * 2);
                if ro == 0 {
                    return (code as u16).wrapping_add(rd_u16(cm, delta + i * 2));
                }
                let at = range + i * 2 + (ro as usize) + 2 * ((code - s) as usize);
                if at + 1 < cm.len() {
                    let gid = rd_u16(cm, at);
                    if gid == 0 {
                        return 0;
                    }
                    return gid.wrapping_add(rd_u16(cm, delta + i * 2));
                }
                return 0;
            }
        }
        0
    }

    fn cmap12(cm: &[u8], off: usize, code: u32) -> Option<u16> {
        let n = rd_u32(cm, off + 12) as usize;
        for i in 0..n {
            let g = off + 16 + i * 12;
            if g + 12 > cm.len() {
                break;
            }
            let s = rd_u32(cm, g);
            let e = rd_u32(cm, g + 4);
            if (s..=e).contains(&code) {
                let st = rd_u32(cm, g + 8);
                return Some((code - s + st) as u16);
            }
        }
        None
    }

    /// Tranche `loca[gid]..loca[gid+1]` dans la table glyf.
    fn loca_span(&self, gid: u16) -> (usize, usize) {
        let loca = match self.table(b"loca") {
            Some(l) => l,
            None => return (0, 0),
        };
        let i = gid as usize;
        let (a, b) = if self.index_to_loc == 0 {
            (
                rd_u16(loca, i * 2) as usize * 2,
                rd_u16(loca, i * 2 + 2) as usize * 2,
            )
        } else {
            (rd_u32(loca, i * 4) as usize, rd_u32(loca, i * 4 + 4) as usize)
        };
        (a.min(b), a.max(b))
    }
}

// ---------------------------------------------------------------------------
// Contours glyf (unités de la face, y vers le HAUT)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Pt {
    x: f64,
    y: f64,
    on: bool,
}

type Contour = Vec<Pt>;

/// Contours du glyphe `gid` (composites résolus, profondeur bornée).
fn contours(f: &Face, gid: u16, depth: u8) -> Vec<Contour> {
    if depth > 4 || gid >= f.num_glyphs {
        return vec![];
    }
    let glo = match f.table(b"glyf") {
        Some(g) => g,
        None => return vec![],
    };
    let (a, b) = f.loca_span(gid);
    if b <= a || b > glo.len() {
        return vec![];
    }
    let g = &glo[a..b];
    if g.len() < 10 {
        return vec![];
    }
    let n_cont = rd_i16(g, 0);
    if n_cont >= 0 {
        simple_glyph(g)
    } else {
        composite_glyph(g, f, depth)
    }
}

fn simple_glyph(g: &[u8]) -> Vec<Contour> {
    let n_cont = match rd_i16(g, 0) {
        n if n >= 0 => n as usize,
        _ => return vec![],
    };
    if n_cont == 0 || g.len() < 10 || g.len() < 10 + n_cont * 2 + 2 {
        return vec![];
    }
    let end_pts: Vec<usize> = (0..n_cont)
        .map(|i| rd_u16(g, 10 + i * 2) as usize)
        .collect();
    // Ordre strictement croissant — sinon glyphe corrompu.
    if end_pts.windows(2).any(|w| w[1] <= w[0]) {
        return vec![];
    }
    let npts = end_pts[n_cont - 1] + 1;
    let ins = rd_u16(g, 10 + n_cont * 2) as usize;
    let mut p = 10 + n_cont * 2 + 2 + ins;
    if p > g.len() || npts == 0 {
        return vec![];
    }
    // Flags (bit 0 : sur courbe ; 0x02/0x04 : coordonnée sur 8 bits ;
    // 0x08 : répétition ; 0x10/0x20 : delta positif ou nul).
    let mut flags = Vec::with_capacity(npts);
    while flags.len() < npts && p < g.len() {
        let fl = g[p];
        flags.push(fl);
        p += 1;
        if fl & 0x08 != 0 {
            if p >= g.len() {
                return vec![];
            }
            let rep = g[p] as usize;
            p += 1;
            for _ in 0..rep {
                if flags.len() >= npts {
                    break;
                }
                flags.push(fl);
            }
        }
    }
    if flags.len() < npts {
        return vec![];
    }
    // Deltas x puis deltas y (accumulés).
    let mut xs = vec![0f64; npts];
    let mut ys = vec![0f64; npts];
    let Some(p) = decode_deltas(g, &flags, npts, p, 0x02, 0x10, &mut xs) else {
        return vec![];
    };
    let Some(p) = decode_deltas(g, &flags, npts, p, 0x04, 0x20, &mut ys) else {
        return vec![];
    };
    let _ = p;
    // Découpage en contours.
    let mut out = Vec::with_capacity(n_cont);
    let mut start = 0usize;
    for &end in &end_pts {
        if end >= npts {
            return vec![];
        }
        let contour: Contour = (start..=end)
            .map(|i| Pt {
                x: xs[i],
                y: ys[i],
                on: flags[i] & 0x01 != 0,
            })
            .collect();
        out.push(contour);
        start = end + 1;
    }
    out
}

/// Deltas de coordonnées : sur 8 bits (signés, ou tout-positif) ou sur
/// 16 bits ; delta nul quando le bit « même » est posé sans « court ».
/// Renvoie l'octet qui suit la séquence (None : tronqué).
fn decode_deltas(
    g: &[u8],
    flags: &[u8],
    n: usize,
    mut p: usize,
    short_bit: u8,
    same_bit: u8,
    out: &mut [f64],
) -> Option<usize> {
    let mut c = 0i64;
    for i in 0..n {
        let fl = flags[i];
        if fl & short_bit != 0 {
            if p + 1 > g.len() {
                return None;
            }
            if fl & same_bit != 0 {
                c += g[p] as i64;
            } else {
                c -= g[p] as i64;
            }
            p += 1;
        } else if fl & same_bit == 0 {
            if p + 2 > g.len() {
                return None;
            }
            c += rd_i16(g, p) as i64;
            p += 2;
        }
        out[i] = c as f64;
    }
    Some(p)
}

/// Composite : composantes récursives avec transformation 2x2 + offset.
fn composite_glyph(g: &[u8], f: &Face, depth: u8) -> Vec<Contour> {
    let mut out: Vec<Contour> = Vec::new();
    let mut p = 10usize; // numberOfContours = -1 + bbox (8 o)
    loop {
        if p + 4 > g.len() {
            break;
        }
        let flags = rd_u16(g, p);
        let gid2 = rd_u16(g, p + 2);
        p += 4;
        // Arguments : offsets si ARGS_ARE_XY_VALUES, sinon appariement de
        // points (rare pour les accents — traité comme décalage nul).
        let mut dx = 0f64;
        let mut dy = 0f64;
        if flags & 0x0001 != 0 {
            if p + 4 > g.len() {
                break;
            }
            let (a1, a2) = (rd_i16(g, p) as i64, rd_i16(g, p + 2) as i64);
            p += 4;
            if flags & 0x0002 != 0 {
                dx = a1 as f64;
                dy = a2 as f64;
            }
        } else {
            if p + 2 > g.len() {
                break;
            }
            let (a1, a2) = (g[p] as i8 as i64, g[p + 1] as i8 as i64);
            p += 2;
            if flags & 0x0002 != 0 {
                dx = a1 as f64;
                dy = a2 as f64;
            }
        }
        let (mut ca, mut cb, mut cc, mut cd) = (1.0, 0.0, 0.0, 1.0);
        if flags & 0x0008 != 0 {
            ca = f2dot14(g, p);
            cd = ca;
            p += 2;
        } else if flags & 0x0040 != 0 {
            ca = f2dot14(g, p);
            cd = f2dot14(g, p + 2);
            p += 4;
        } else if flags & 0x0080 != 0 {
            ca = f2dot14(g, p);
            cb = f2dot14(g, p + 2);
            cc = f2dot14(g, p + 4);
            cd = f2dot14(g, p + 6);
            p += 8;
        }
        for mut cont in contours(f, gid2, depth + 1) {
            for pt in cont.iter_mut() {
                let nx = ca * pt.x + cc * pt.y + dx;
                let ny = cb * pt.x + cd * pt.y + dy;
                pt.x = nx;
                pt.y = ny;
            }
            out.push(cont);
        }
        if flags & 0x0020 == 0 {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Aplatissement quadratique → segments (pixels, y vers le BAS)
// ---------------------------------------------------------------------------

/// Segment [x0, y0, x1, y1] en espace bitmap (pixels, y vers le bas).
type Edge = [f64; 4];

/// Aplatit les contours d'un glyphe en segments device (x = droite,
/// y = bas, origine : ligne de base du texte). Les points hors courbe
/// sont des contrôles quadratiques ; le milieu implicite relie deux
/// contrôles consécutifs ; le contour est fermé (dernier segment →
/// premier point).
fn flatten(conts: &[Contour], scale: f64) -> Vec<Vec<Edge>> {
    let mut out = Vec::with_capacity(conts.len());
    for cont in conts {
        let n = cont.len();
        if n < 2 {
            continue;
        }
        let dev = |p: &Pt| (p.x * scale, -p.y * scale);
        let do_quad = |x0: f64, y0: f64, cx: f64, cy: f64, x2: f64, y2: f64,
                       edges: &mut Vec<Edge>| {
            let d0 = (cx - x0).hypot(cy - y0);
            let d1 = (x2 - cx).hypot(y2 - cy);
            let steps = (((d0 + d1) / 2.5).ceil().round() as usize).clamp(2, 24);
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                let s = 1.0 - t;
                let x = s * s * x0 + 2.0 * s * t * cx + t * t * x2;
                let y = s * s * y0 + 2.0 * s * t * cy + t * t * y2;
                edges.push([x0, y0, x, y]);
            }
        };
        let dev_quad = |a: &(f64, f64), c: &(f64, f64), b: &(f64, f64),
                        edges: &mut Vec<Edge>| {
            do_quad(a.0, a.1, c.0, c.1, b.0, b.1, edges);
        };
        let mut edges: Vec<Edge> = Vec::with_capacity(n * 2);
        match cont.iter().position(|p| p.on) {
            Some(i0) => {
                // Rotation implicite : on part DU point sur courbe
                // d'indice i0, on ferme au retour (cyclique).
                let start = dev(&cont[i0]);
                let mut cur = start;
                let mut ctrl: Option<(f64, f64)> = None;
                for k in 1..=n {
                    let (p, on) = {
                        let q = &cont[(i0 + k) % n];
                        (dev(q), q.on)
                    };
                    if on {
                        match ctrl.take() {
                            Some(c) => dev_quad(&cur, &c, &p, &mut edges),
                            None => edges.push([cur.0, cur.1, p.0, p.1]),
                        }
                        cur = p;
                    } else if let Some(c) = ctrl.replace(p) {
                        let mid = ((c.0 + p.0) / 2.0, (c.1 + p.1) / 2.0);
                        dev_quad(&cur, &c, &mid, &mut edges);
                        cur = mid;
                    }
                }
                if let Some(c) = ctrl {
                    dev_quad(&cur, &c, &start, &mut edges);
                }
            }
            None => {
                // aucun point sur courbe (contour exotique) : les
                // milieux implicites m_i = mid(p_i, p_{i+1}) sont les
                // ancrs, chaque p_i est le contrôle entre m_i et m_{i+1}.
                let m: Vec<(f64, f64)> = (0..n)
                    .map(|i| {
                        let a = dev(&cont[i]);
                        let b = dev(&cont[(i + 1) % n]);
                        ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
                    })
                    .collect();
                for i in 0..n {
                    let c = dev(&cont[i]);
                    dev_quad(&m[i], &c, &m[(i + 1) % n], &mut edges);
                }
            }
        }
        out.push(edges);
    }
    out
}

// ---------------------------------------------------------------------------
// Pixellisation (remplissage non-nul, échantillonnage vertical ×4)
// ---------------------------------------------------------------------------

const SUBSAMPLES: usize = 4;

/// Boîte englobante + remplissage des segments → couverture 0..255.
fn rasterize(edges: &[Vec<Edge>], size: u32) -> GlyphBitmap {
    // Boîte des sommets (contenue au pixel près, 1 px de marge AA).
    let empty = GlyphBitmap {
        w: 0,
        h: 0,
        left: 0,
        top: 0,
        cov: vec![],
    };
    let mut minx = f64::INFINITY;
    let mut miny = f64::INFINITY;
    let mut maxx = f64::NEG_INFINITY;
    let mut maxy = f64::NEG_INFINITY;
    let mut total_edges = 0usize;
    for seg in edges.iter().flatten() {
        minx = minx.min(seg[0].min(seg[2]));
        maxx = maxx.max(seg[0].max(seg[2]));
        miny = miny.min(seg[1].min(seg[3]));
        maxy = maxy.max(seg[1].min(seg[3]));
        total_edges += 1;
    }
    if total_edges == 0 {
        return empty;
    }
    let pad = 1.5_f64;
    let left = (minx - pad).floor() as i32;
    let top = (miny - pad).floor() as i32;
    let w = ((maxx + pad).ceil() as i32 - left).max(1);
    let h = ((maxy + pad).ceil() as i32 - top).max(1);
    // Garde-fou : glyphe pathologique (face corrompue).
    let max_dim = (size as i32 * 8).clamp(64, 384);
    if w > max_dim || h > max_dim {
        return empty;
    }
    let mut cov = vec![0u8; (w * h) as usize];
    let mut row = vec![0f64; w as usize];
    let mut cross: Vec<(f64, i32)> = Vec::with_capacity(total_edges);
    for y in 0..h {
        row.fill(0.0);
        for s in 0..SUBSAMPLES {
            let sy = top as f64 + y as f64 + (s as f64 + 0.5) / SUBSAMPLES as f64;
            cross.clear();
            for seg in edges.iter().flatten() {
                let (x0, y0, x1, y1) = (seg[0], seg[1], seg[2], seg[3]);
                if y0 == y1 {
                    continue;
                }
                if (y0 <= sy && sy < y1) || (y1 <= sy && sy < y0) {
                    let t = (sy - y0) / (y1 - y0);
                    let x = x0 + t * (x1 - x0);
                    cross.push((x, if y1 > y0 { 1 } else { -1 }));
                }
            }
            if cross.is_empty() {
                continue;
            }
            cross.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut winding = 0i32;
            let mut prev = 0f64;
            for (x, d) in cross.iter().copied() {
                if winding != 0 {
                    fill_span(&mut row, prev, x, left, w);
                }
                winding += d;
                prev = x;
            }
        }
        // Quantification de la couverture de la ligne.
        for (i, v) in row.iter().enumerate() {
            let a = (*v * 255.0).round().clamp(0.0, 255.0) as u8;
            let _ = i;
            cov[((y as usize) * w as usize) + i] = a;
        }
    }
    GlyphBitmap { w, h, left, top, cov }
}

/// Recouvrement horizontal exact d'un span [min(xa,xb), max(xa,xb))
/// réparti sur les pixels de la ligne `row` (coords bitmap locales).
fn fill_span(row: &mut [f64], xa: f64, xb: f64, left: i32, w: i32) {
    if xa == xb {
        return;
    }
    let (a, b) = if xa < xb { (xa, xb) } else { (xb, xa) };
    let la = a - left as f64;
    let lb = b - left as f64;
    if lb <= 0.0 || la >= w as f64 {
        return;
    }
    let col_a = la.floor().max(0.0) as usize;
    let col_b = lb.ceil().min(w as f64) as usize;
    for col in col_a..col_b.min(w as usize) {
        let lo = la.max(col as f64);
        let hi = lb.min(col as f64 + 1.0);
        if hi > lo {
            row[col] += (hi - lo) / SUBSAMPLES as f64;
        }
    }
}

// ---------------------------------------------------------------------------
// API publiques (texte)
// ---------------------------------------------------------------------------

/// Faces — parse une fois, cache de glyphes ensuite.
struct Cache {
    glyphs: HashMap<(u32, u32, u8), Arc<GlyphBitmap>>,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| Mutex::new(Cache { glyphs: HashMap::new() }))
}

fn face_of(weight: Weight) -> Option<&'static Face> {
    let f = faces().as_ref()?;
    Some(match weight {
        Weight::Regular => &f.reg,
        Weight::Bold => &f.bold,
    })
}

/// Bitmap antialiasé d'un caractère à la taille et au poids donnés
/// (rebâtir le cache : un glyphe = une fois).
pub fn glyph(ch: char, size: u32, weight: Weight) -> Option<Arc<GlyphBitmap>> {
    let face = face_of(weight)?;
    let key = (ch as u32, size, weight as u8);
    let mut cache = cache().lock().expect("cache verrouillé");
    if cache.glyphs.len() > 8192 {
        cache.glyphs.clear(); // plancher mémoire souple
    }
    Some(
        cache
            .glyphs
            .entry(key)
            .or_insert_with(|| {
                let gid = face.glyph_id(ch);
                let conts = contours(face, gid, 0);
                let edges = flatten(&conts, face.scale(size));
                Arc::new(rasterize(&edges, size))
            })
            .clone(),
    )
}

/// Hauteur de la ligne de base du design : ascent de la face.
pub fn text_ascent(size: u32) -> i32 {
    face_of(Weight::Regular)
        .map(|f| f.v_metrics(size).0)
        .unwrap_or(size as i32)
}

/// Hauteur de ligne (ascent + descent, metrics hhea).
pub fn text_height(size: u32) -> i32 {
    face_of(Weight::Regular)
        .map(|f| f.v_metrics(size).0 + f.v_metrics(size).1)
        .unwrap_or(size as i32 + 2)
}

/// Largeur mesurée d'une ligne de texte (somme des avancements — sans
/// crénage : les libellés de l'UI en restent propres).
pub fn text_width(text: &str, size: u32, weight: Weight) -> i32 {
    let Some(face) = face_of(weight) else {
        return 0;
    };
    text.chars().map(|ch| face.advance(face.glyph_id(ch), size)).sum()
}

/// Même chose avec interlettrage constant `tracking` px (ÉPINGLÉES et
/// libellés capitales espacés du design).
pub fn text_width_tracked(text: &str, size: u32, weight: Weight, tracking: i32) -> i32 {
    let n = text.chars().count();
    (n as i32).checked_sub(1).map_or_else(
        || text_width(text, size, weight),
        |extra| text_width(text, size, weight) + extra as i32 * tracking,
    )
}

/// Variante baseligne avec interlettrage (dessin caractères par caractères).
pub fn draw_text_baseline_tracked(
    canvas: &mut [u8],
    cw: i32,
    ch: i32,
    text: &str,
    x0: i32,
    baseline: i32,
    size: u32,
    weight: Weight,
    tracking: i32,
    color: u32,
) {
    let Some(face) = face_of(weight) else { return };
    let mut pen = x0;
    for c in text.chars() {
        let gid = face.glyph_id(c);
        if let Some(g) = glyph(c, size, weight) {
            if !g.is_empty() {
                blit_bitmap(canvas, cw, ch, pen + g.left, baseline + g.top, &g, color);
            }
        }
        pen += face.advance(gid, size) + tracking;
    }
}

/// Dessine le texte à l'angle SUPÉRIEUR GAUCHE de sa boîte de ligne
/// (baseligne = y_top + ascent). Ne dessine rien en repli (face
/// absente).
pub fn draw_text(
    canvas: &mut [u8],
    cw: i32,
    chh: i32,
    text: &str,
    x0: i32,
    y_top: i32,
    size: u32,
    weight: Weight,
    color: u32,
) {
    let Some(face) = face_of(weight) else { return };
    let asc = face.v_metrics(size).0;
    draw_text_baseline(canvas, cw, chh, text, x0, y_top + asc, size, weight, color);
}

/// Dessine le texte à la ligne de base (pas d'interlettrage).
pub fn draw_text_baseline(
    canvas: &mut [u8],
    cw: i32,
    chh: i32,
    text: &str,
    x0: i32,
    baseline: i32,
    size: u32,
    weight: Weight,
    color: u32,
) {
    let Some(face) = face_of(weight) else { return };
    let mut pen = x0;
    for ch in text.chars() {
        let gid = face.glyph_id(ch);
        let adv = face.advance(gid, size);
        if let Some(g) = glyph(ch, size, weight) {
            if !g.is_empty() {
                blit_bitmap(canvas, cw, chh, pen + g.left, baseline + g.top, &g, color);
            }
        }
        pen += adv;
    }
}

/// Copie une bitmap de glyphe (couverture 0..255) sur le canvas.
fn blit_bitmap(
    canvas: &mut [u8],
    cw: i32,
    chh: i32,
    x0: i32,
    y0: i32,
    g: &GlyphBitmap,
    color: u32,
) {
    for row in 0..g.h {
        for col in 0..g.w {
            let a = g.cov[(row * g.w + col) as usize] as f64 / 255.0;
            if a > 0.0 {
                crate::raster::blend(
                    canvas,
                    cw,
                    chh,
                    x0 + col,
                    y0 + row,
                    color,
                    a,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests (peuvent charger les faces — dépendance de fichier embarqué)
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Tests (faces embarquées : parse + pixellisation)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::fill_rect;

    /// Décompte de pixels « allumés » (rouge pur du fond attenué par la
    /// couverture) — proxy de surface de glyphe.
    fn lit_pixels(canvas: &[u8]) -> usize {
        canvas.chunks_exact(4).filter(|p| p[2] > 8).count()
    }

    #[test]
    fn faces_parse() {
        assert!(ready(), "face Noto Sans Regular/Bold parse");
        assert!(face_of(Weight::Regular).is_some());
        assert!(face_of(Weight::Bold).is_some());
    }

    #[test]
    fn glyphes_minuscules_et_accentués() {
        assert!(ready());
        for ch in ['A', 'a', 'é', 'È', 'ç', 'ù', 'ô', 'L', 'F', '0', '9', '%'] {
            assert!(glyph(ch, 12, Weight::Regular).is_some(), "glyphe {ch}");
        }
        // é = composite : couverture au-dessus de la x-hauche — h étendue
        let g = glyph('é', 14, Weight::Regular).unwrap();
        assert!(g.w > 4 && g.h > 12, "é {}x{}", g.w, g.h);
    }

    #[test]
    fn cache_identique_par_cle() {
        let a = glyph('A', 14, Weight::Bold).expect("glyphe");
        let b = glyph('A', 14, Weight::Bold).expect("glyphe");
        assert!(Arc::ptr_eq(&a, &b), "cache (caractère, taille, poids)");
    }

    #[test]
    fn largeurs_et_poids() {
        assert!(text_width("Fichiers", 12, Weight::Regular) > 30);
        assert!(text_width("é", 12, Weight::Regular) > 0);
        assert_ne!(
            text_width("Terminal", 12, Weight::Regular),
            text_width("Terminal", 12, Weight::Bold),
            "poids = face différente"
        );
        assert_eq!(
            text_width("AB", 12, Weight::Regular) + 3,
            text_width_tracked("AB", 12, Weight::Regular, 3),
            "interlettrage = par caractère"
        );
    }

    #[test]
    fn draw_text_allume_le_baseline() {
        let (w, h) = (64, 20);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rect(&mut canvas, w, h, 0, 0, w, h, 0xFF_FF_00_00);
        draw_text(&mut canvas, w, h, "F", 29, 1, 14, Weight::Bold, 0xFF_FF_FF_FF);
        let plein = lit_pixels(&canvas);
        assert!(plein > 10, "glyphe « F » dessiné ({} pixels)", plein);
    }

    #[test]
    fn baseline_et_hauteur_ligne() {
        assert!(text_height(12) > 12);
        assert!(text_ascent(12) > 6);
    }
}

