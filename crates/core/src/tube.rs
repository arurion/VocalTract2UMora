// SPDX-License-Identifier: GPL-3.0-or-later
//! Tube model: a chain of short tube sections (CGS units: cm, cm^2, cm^3).
//!
//! The layout and default values follow VocalTractLab's `Tube` (oral-only subset: trachea,
//! two glottal sections, pharynx and mouth; the nasal cavity, sinuses and piriform fossa are
//! not included yet).

pub const NUM_TRACHEA: usize = 23;
pub const NUM_GLOTTIS: usize = 2;
pub const NUM_PHARYNX: usize = 16;
pub const NUM_MOUTH: usize = 24;
pub const NUM_SUPRAGLOTTAL: usize = NUM_PHARYNX + NUM_MOUTH;

pub const FIRST_TRACHEA: usize = 0;
pub const LAST_TRACHEA: usize = NUM_TRACHEA - 1;
pub const LOWER_GLOTTIS: usize = NUM_TRACHEA;
pub const UPPER_GLOTTIS: usize = NUM_TRACHEA + 1;
pub const FIRST_PHARYNX: usize = NUM_TRACHEA + NUM_GLOTTIS;
pub const FIRST_MOUTH: usize = FIRST_PHARYNX + NUM_PHARYNX;
pub const LAST_MOUTH: usize = FIRST_MOUTH + NUM_MOUTH - 1;
pub const NUM_SECTIONS: usize = NUM_TRACHEA + NUM_GLOTTIS + NUM_SUPRAGLOTTAL;

/// Nasal cavity: 19 sections of 0.6 cm each (VTL `Tube::initNasalCavity`).
pub const NUM_NOSE: usize = 19;
pub const DEFAULT_NASAL_CAVITY_LENGTH_CM: f64 = 11.4;
/// Number of nasal sections whose area is interpolated from the velum opening.
const NUM_VELUM_COUPLED: usize = 4;
const NOSE_AREA_CM2: [f64; NUM_NOSE] = [
    1.63, 2.07, 2.72, 3.59, 4.24, 3.26, 3.04, 3.04, 2.72, 2.5, 2.39, 2.39, 1.85, 0.76, 1.41, 1.74, 1.30, 1.74, 0.76,
];

pub const MIN_AREA_CM2: f64 = 0.01e-2;
pub const STANDARD_WALL_MASS: f64 = 2.4;
pub const STANDARD_WALL_RESISTANCE: f64 = 5000.0;
pub const STANDARD_WALL_STIFFNESS: f64 = 100_000.0;

/// Which articulator a section lies in (used later for fricative noise sources).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Articulator {
    VocalFolds,
    Tongue,
    LowerIncisors,
    LowerLip,
    #[default]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub area_cm2: f64,
    pub length_cm: f64,
    pub wall_mass: f64,
    pub wall_resistance: f64,
    pub wall_stiffness: f64,
    pub articulator: Articulator,
}

impl Section {
    pub fn volume_cm3(&self) -> f64 {
        self.area_cm2 * self.length_cm
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tube {
    pub sections: Vec<Section>,
    /// Position of the upper incisors, measured from the upper end of the glottis (cm).
    pub teeth_position_cm: f64,
    /// Aspiration noise strength at the glottis (dB). VTL default: -40 dB.
    pub aspiration_strength_db: f64,
    /// Elevation of the tongue-tip sides: 1 = fricative-like, 0 = plosive-like, < 0 = lateral.
    pub tongue_tip_side_elevation: f64,
    /// Nasal cavity sections (first one is at the velum port). Sinuses are not modelled yet.
    pub nose: Vec<Section>,
    /// Nasal cavity length (cm); the 0.6 cm section lengths scale with length / 11.4.
    pub nasal_cavity_length_cm: f64,
    /// Velum opening area (cm^2) last passed to `set_velum_opening`.
    pub velum_opening_cm2: f64,
}

impl Default for Tube {
    /// Default trachea (4, 3, then 2.5 cm^2, 1 cm per section), glottis 0.1 cm^2 x 0.3 cm,
    /// and a 16 cm supraglottal tube of 4 cm^2.
    fn default() -> Self {
        let mut sections = Vec::with_capacity(NUM_SECTIONS);
        for i in 0..NUM_TRACHEA {
            let area = match i { 0 => 4.0, 1 => 3.0, _ => 2.5 };
            sections.push(Section {
                area_cm2: area, length_cm: 1.0, wall_mass: 0.25, wall_resistance: 1000.0,
                wall_stiffness: STANDARD_WALL_STIFFNESS, articulator: Articulator::Other,
            });
        }
        for _ in 0..NUM_GLOTTIS {
            sections.push(Section {
                area_cm2: 0.1, length_cm: 0.3, wall_mass: STANDARD_WALL_MASS,
                wall_resistance: STANDARD_WALL_RESISTANCE, wall_stiffness: STANDARD_WALL_STIFFNESS,
                articulator: Articulator::VocalFolds,
            });
        }
        for _ in 0..NUM_SUPRAGLOTTAL {
            sections.push(Section {
                area_cm2: 4.0, length_cm: 16.0 / NUM_SUPRAGLOTTAL as f64, wall_mass: STANDARD_WALL_MASS,
                wall_resistance: STANDARD_WALL_RESISTANCE, wall_stiffness: STANDARD_WALL_STIFFNESS,
                articulator: Articulator::Other,
            });
        }
        let mut tube = Tube {
            sections, teeth_position_cm: 15.0, aspiration_strength_db: -40.0, tongue_tip_side_elevation: 0.0,
            nose: Vec::new(), nasal_cavity_length_cm: 0.0, velum_opening_cm2: 0.0,
        };
        tube.init_nasal_cavity(DEFAULT_NASAL_CAVITY_LENGTH_CM);
        tube.set_velum_opening(0.0);
        tube
    }
}

impl Tube {
    /// (Re)builds the nasal cavity for the given length (VTL `initNasalCavity`): fixed area
    /// profile, 0.6 cm sections scaled by length / 11.4. Call `set_velum_opening` afterwards.
    pub fn init_nasal_cavity(&mut self, length_cm: f64) {
        self.nasal_cavity_length_cm = length_cm;
        let factor = length_cm / DEFAULT_NASAL_CAVITY_LENGTH_CM;
        self.nose = (0..NUM_NOSE)
            .map(|i| Section {
                area_cm2: NOSE_AREA_CM2[i], length_cm: 0.6 * factor, wall_mass: STANDARD_WALL_MASS,
                wall_resistance: STANDARD_WALL_RESISTANCE, wall_stiffness: STANDARD_WALL_STIFFNESS,
                articulator: Articulator::Other,
            })
            .collect();
        self.set_velum_opening(self.velum_opening_cm2);
    }

    /// Sets the velo-pharyngeal port area (VTL `setVelumOpening`): the first four nasal
    /// sections are interpolated quadratically from the opening to the area of section 4.
    pub fn set_velum_opening(&mut self, opening_cm2: f64) {
        self.velum_opening_cm2 = opening_cm2;
        let n = NUM_VELUM_COUPLED;
        let target = NOSE_AREA_CM2[n];
        for i in 0..n {
            let a = opening_cm2 + (i * i) as f64 * (target - opening_cm2) / (n * n) as f64;
            self.nose[i].area_cm2 = a.max(MIN_AREA_CM2);
        }
    }

    /// Sets both glottal sections to the same area.
    pub fn set_glottis_area(&mut self, area_cm2: f64) {
        for i in [LOWER_GLOTTIS, UPPER_GLOTTIS] {
            self.sections[i].area_cm2 = area_cm2.max(MIN_AREA_CM2);
            self.sections[i].length_cm = 0.3;
        }
    }

    /// Sets the lengths and areas of the two glottal sections (from the glottis model).
    pub fn set_glottis_geometry(&mut self, length_cm: [f64; 2], area_cm2: [f64; 2]) {
        for (k, i) in [LOWER_GLOTTIS, UPPER_GLOTTIS].into_iter().enumerate() {
            self.sections[i].length_cm = length_cm[k];
            self.sections[i].area_cm2 = area_cm2[k].max(MIN_AREA_CM2);
        }
    }

    /// Sets the 40 supraglottal sections (pharynx then mouth).
    pub fn set_supraglottal(&mut self, length_cm: &[f64], area_cm2: &[f64]) {
        assert_eq!(length_cm.len(), NUM_SUPRAGLOTTAL);
        assert_eq!(area_cm2.len(), NUM_SUPRAGLOTTAL);
        for k in 0..NUM_SUPRAGLOTTAL {
            let s = &mut self.sections[FIRST_PHARYNX + k];
            s.length_cm = length_cm[k];
            s.area_cm2 = area_cm2[k].max(MIN_AREA_CM2);
        }
    }

    /// Assigns the articulator of each of the 40 supraglottal sections (used to place noise sources).
    pub fn set_articulators(&mut self, tags: &[Articulator]) {
        assert_eq!(tags.len(), NUM_SUPRAGLOTTAL);
        for k in 0..NUM_SUPRAGLOTTAL {
            self.sections[FIRST_PHARYNX + k].articulator = tags[k];
        }
    }

    /// Section start positions (cm) along the tract, as in VTL: 0 is the upper end of the
    /// glottis, positions increase towards the lips and are negative in the glottis and trachea.
    pub fn positions(&self) -> Vec<f64> {
        let mut pos = vec![0.0; NUM_SECTIONS];
        let mut p = 0.0;
        for i in (0..FIRST_PHARYNX).rev() {
            p -= self.sections[i].length_cm;
            pos[i] = p;
        }
        let mut p = 0.0;
        for i in FIRST_PHARYNX..NUM_SECTIONS {
            pos[i] = p;
            p += self.sections[i].length_cm;
        }
        pos
    }

    /// Linear interpolation of the supraglottal sections between two tubes (the glottal
    /// sections stay as they are). Areas and lengths are interpolated; the articulator of the
    /// nearer tube is used.
    pub fn interpolate_supraglottal(&mut self, left: &Tube, right: &Tube, ratio: f64) {
        let ratio1 = 1.0 - ratio;
        for i in FIRST_PHARYNX..NUM_SECTIONS {
            let (l, r) = (&left.sections[i], &right.sections[i]);
            let s = &mut self.sections[i];
            s.area_cm2 = (ratio1 * l.area_cm2 + ratio * r.area_cm2).max(MIN_AREA_CM2);
            s.length_cm = ratio1 * l.length_cm + ratio * r.length_cm;
            s.articulator = if ratio < 0.5 { l.articulator } else { r.articulator };
        }
        self.teeth_position_cm = ratio1 * left.teeth_position_cm + ratio * right.teeth_position_cm;
        self.tongue_tip_side_elevation = ratio1 * left.tongue_tip_side_elevation + ratio * right.tongue_tip_side_elevation;
    }

    /// A uniform supraglottal tube of the given total length and area.
    pub fn set_uniform_supraglottal(&mut self, total_length_cm: f64, area_cm2: f64) {
        let l = vec![total_length_cm / NUM_SUPRAGLOTTAL as f64; NUM_SUPRAGLOTTAL];
        let a = vec![area_cm2; NUM_SUPRAGLOTTAL];
        self.set_supraglottal(&l, &a);
    }
}
