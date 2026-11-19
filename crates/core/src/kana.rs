// SPDX-License-Identifier: GPL-3.0-or-later
//! Japanese kana: splitting text into morae, mapping them to consonant + vowel, and Shift_JIS
//! encoding of the aliases for oto.ini (UTAU reads oto.ini as Shift_JIS).

/// A mora as it is spoken: consonant symbol (as in `mora::consonant_table`), vowel, and whether
/// it is palatalised (きゃ ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mora {
    /// The mora as written, in hiragana (used as the oto alias).
    pub kana: String,
    pub consonant: &'static str,
    /// "a", "i", "u", "e", "o".
    pub vowel: &'static str,
    pub palatalised: bool,
}

/// Base kana -> (consonant symbol, vowel).
fn base(c: char) -> Option<(&'static str, &'static str)> {
    Some(match c {
        'あ' => ("", "a"), 'い' => ("", "i"), 'う' => ("", "u"), 'え' => ("", "e"), 'お' => ("", "o"),
        'か' => ("k", "a"), 'き' => ("k", "i"), 'く' => ("k", "u"), 'け' => ("k", "e"), 'こ' => ("k", "o"),
        'が' => ("g", "a"), 'ぎ' => ("g", "i"), 'ぐ' => ("g", "u"), 'げ' => ("g", "e"), 'ご' => ("g", "o"),
        'さ' => ("s", "a"), 'し' => ("sh", "i"), 'す' => ("s", "u"), 'せ' => ("s", "e"), 'そ' => ("s", "o"),
        'ざ' => ("z", "a"), 'じ' => ("j", "i"), 'ず' => ("z", "u"), 'ぜ' => ("z", "e"), 'ぞ' => ("z", "o"),
        'た' => ("t", "a"), 'ち' => ("ch", "i"), 'つ' => ("ts", "u"), 'て' => ("t", "e"), 'と' => ("t", "o"),
        'だ' => ("d", "a"), 'ぢ' => ("j", "i"), 'づ' => ("z", "u"), 'で' => ("d", "e"), 'ど' => ("d", "o"),
        'な' => ("n", "a"), 'に' => ("n", "i"), 'ぬ' => ("n", "u"), 'ね' => ("n", "e"), 'の' => ("n", "o"),
        'は' => ("h", "a"), 'ひ' => ("hy", "i"), 'ふ' => ("f", "u"), 'へ' => ("h", "e"), 'ほ' => ("h", "o"),
        'ば' => ("b", "a"), 'び' => ("b", "i"), 'ぶ' => ("b", "u"), 'べ' => ("b", "e"), 'ぼ' => ("b", "o"),
        'ぱ' => ("p", "a"), 'ぴ' => ("p", "i"), 'ぷ' => ("p", "u"), 'ぺ' => ("p", "e"), 'ぽ' => ("p", "o"),
        'ま' => ("m", "a"), 'み' => ("m", "i"), 'む' => ("m", "u"), 'め' => ("m", "e"), 'も' => ("m", "o"),
        'や' => ("y", "a"), 'ゆ' => ("y", "u"), 'よ' => ("y", "o"),
        'ら' => ("r", "a"), 'り' => ("r", "i"), 'る' => ("r", "u"), 'れ' => ("r", "e"), 'ろ' => ("r", "o"),
        'わ' => ("w", "a"), 'を' => ("", "o"),
        _ => return None,
    })
}

/// Katakana to hiragana (the long-vowel mark and other characters are unchanged).
pub fn to_hiragana(s: &str) -> String {
    s.chars().map(|c| if ('ァ'..='ヶ').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap() } else { c }).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanaError {
    /// A character that cannot be spoken by the synthesizer (yet).
    Unsupported(char),
    /// Small kana or a mark with nothing to attach to.
    Dangling(char),
}

impl std::fmt::Display for KanaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KanaError::Unsupported(c) => write!(f, "「{c}」は未対応です"),
            KanaError::Dangling(c) => write!(f, "「{c}」の前に付く文字がありません"),
        }
    }
}

/// Splits kana text into morae. Combinations with small ゃゅょ become one palatalised mora
/// (きゃ = k + a, しゃ = sh + a), and the long-vowel mark ー repeats the previous vowel.
/// Spaces and line breaks are ignored.
pub fn split_morae(text: &str) -> Result<Vec<Mora>, KanaError> {
    let chars: Vec<char> = to_hiragana(text).chars().filter(|c| !c.is_whitespace()).collect();
    let mut out: Vec<Mora> = vec![];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == 'ー' {
            let prev = out.last().ok_or(KanaError::Dangling(c))?;
            out.push(Mora { kana: "ー".to_string(), consonant: "", vowel: prev.vowel, palatalised: false });
            i += 1;
            continue;
        }
        let Some((cons, vowel)) = base(c) else {
            return Err(if "ゃゅょぁぃぅぇぉっん".contains(c) { KanaError::Dangling(c) } else { KanaError::Unsupported(c) });
        };
        let small = chars.get(i + 1).copied().filter(|n| matches!(n, 'ゃ' | 'ゅ' | 'ょ'));
        if let Some(s) = small {
            let v = match s { 'ゃ' => "a", 'ゅ' => "u", _ => "o" };
            // Consonants that are already palatal (し ち じ) just change the vowel.
            let palatal_already = matches!(cons, "sh" | "ch" | "j");
            out.push(Mora { kana: format!("{c}{s}"), consonant: cons, vowel: v, palatalised: !palatal_already });
            i += 2;
        } else {
            out.push(Mora { kana: c.to_string(), consonant: cons, vowel, palatalised: false });
            i += 1;
        }
    }
    Ok(out)
}

/// Romaji of a mora for file names (Hepburn-like, ASCII only).
pub fn romaji(m: &Mora) -> String {
    if m.kana == "ー" { return "-".to_string(); }
    let c = match m.consonant { "hy" => "h", "ng" => "ng", x => x };
    let pal = if m.palatalised { "y" } else { "" };
    if matches!(m.consonant, "sh" | "ch" | "j") && matches!(m.vowel, "a" | "u" | "o") && m.kana.chars().count() == 2 {
        return format!("{c}{}", m.vowel);
    }
    match (m.consonant, m.vowel) {
        ("sh", "i") | ("ch", "i") | ("j", "i") | ("ts", "u") => format!("{c}{}", m.vowel),
        ("f", "u") => "fu".to_string(),
        ("y", v) => format!("y{v}"),
        ("w", v) => format!("w{v}"),
        (c2, v) => format!("{c2}{pal}{v}"),
    }
}

/// Encodes text for oto.ini in Shift_JIS (CP932). Only ASCII, hiragana, katakana, the
/// long-vowel mark and a few symbols are supported; others are rejected.
pub fn to_shift_jis(s: &str) -> Result<Vec<u8>, char> {
    let mut out = Vec::with_capacity(s.len() * 2);
    for c in s.chars() {
        let u = c as u32;
        match c {
            '\u{0}'..='\u{7f}' => out.push(u as u8),
            'ぁ'..='ん' => { let code = 0x829F + (u - 0x3041); out.extend([(code >> 8) as u8, (code & 0xff) as u8]); }
            'ァ'..='ヶ' => {
                let mut code = 0x8340 + (u - 0x30A1);
                if u >= 0x30E0 { code += 1; } // 0x837F is skipped
                out.extend([(code >> 8) as u8, (code & 0xff) as u8]);
            }
            'ー' => out.extend([0x81, 0x5B]),
            '・' => out.extend([0x81, 0x45]),
            '　' => out.extend([0x81, 0x40]),
            _ => return Err(c),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kana(s: &str) -> Vec<String> { split_morae(s).unwrap().iter().map(|m| format!("{}{}{}", m.consonant, if m.palatalised { "'" } else { "" }, m.vowel)).collect() }

    #[test]
    fn splits_plain_and_voiced_morae() {
        assert_eq!(kana("かきくけこ"), ["ka", "ki", "ku", "ke", "ko"]);
        assert_eq!(kana("しちつふじ"), ["shi", "chi", "tsu", "fu", "ji"]);
        assert_eq!(kana("あをわ"), ["a", "o", "wa"]);
        assert_eq!(kana("ぱぴぷ"), ["pa", "pi", "pu"]);
    }

    #[test]
    fn combines_small_kana_into_one_palatalised_mora() {
        assert_eq!(kana("きゃきゅきょ"), ["k'a", "k'u", "k'o"]);
        // し ち じ are already palatal: only the vowel changes.
        assert_eq!(kana("しゃちゅじょ"), ["sha", "chu", "jo"]);
        let m = split_morae("りゃ").unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kana, "りゃ");
    }

    #[test]
    fn katakana_spaces_and_long_vowels() {
        assert_eq!(kana("カ キ"), ["ka", "ki"]);
        let m = split_morae("かー").unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!((m[1].consonant, m[1].vowel), ("", "a"));
    }

    #[test]
    fn rejects_what_it_cannot_speak() {
        assert_eq!(split_morae("ゃ"), Err(KanaError::Dangling('ゃ')));
        assert_eq!(split_morae("a"), Err(KanaError::Unsupported('a')));
        assert_eq!(split_morae("ー"), Err(KanaError::Dangling('ー')));
    }

    #[test]
    fn romaji_for_file_names() {
        let r = |s: &str| split_morae(s).unwrap().iter().map(romaji).collect::<Vec<_>>().join("");
        assert_eq!(r("かしゃちゅきゃ"), "kashachukya");
        assert_eq!(r("ふじつ"), "fujitsu");
        assert!(r("あいうえおやゆよ").is_ascii());
    }

    #[test]
    fn shift_jis_encodes_kana() {
        assert_eq!(to_shift_jis("あ").unwrap(), [0x82, 0xA0]);
        assert_eq!(to_shift_jis("ん").unwrap(), [0x82, 0xF1]);
        assert_eq!(to_shift_jis("ア").unwrap(), [0x83, 0x41]);
        assert_eq!(to_shift_jis("ン").unwrap(), [0x83, 0x93]);
        assert_eq!(to_shift_jis("ミ").unwrap(), [0x83, 0x7E]); // just before the skipped 0x7F
        assert_eq!(to_shift_jis("ム").unwrap(), [0x83, 0x80]); // just after it
        assert_eq!(to_shift_jis("ー").unwrap(), [0x81, 0x5B]);
        assert_eq!(to_shift_jis("a_1").unwrap(), b"a_1");
        assert_eq!(to_shift_jis("漢"), Err('漢'));
    }
}
