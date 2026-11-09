//! UTAU oto.ini entry. Times are in milliseconds.

#[derive(Debug, Clone, PartialEq)]
pub struct OtoEntry {
    pub wav: String,
    pub alias: String,
    /// Left blank (offset).
    pub offset: f64,
    /// Fixed (consonant) region.
    pub consonant: f64,
    /// Right blank; negative values are measured from the end of the file.
    pub cutoff: f64,
    /// Preutterance.
    pub preutterance: f64,
    pub overlap: f64,
}

impl OtoEntry {
    /// `wav=alias,offset,consonant,cutoff,preutterance,overlap`
    pub fn to_line(&self) -> String {
        format!(
            "{}={},{:.1},{:.1},{:.1},{:.1},{:.1}",
            self.wav, self.alias, self.offset, self.consonant, self.cutoff, self.preutterance, self.overlap
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_line() {
        let e = OtoEntry {
            wav: "ka.wav".into(), alias: "か".into(), offset: 10.0,
            consonant: 90.0, cutoff: -200.0, preutterance: 60.0, overlap: 20.0,
        };
        assert_eq!(e.to_line(), "ka.wav=か,10.0,90.0,-200.0,60.0,20.0");
    }
}
