// SPDX-License-Identifier: GPL-3.0-or-later
//! Speaker file (`.speaker`) of VocalTractLab: the speaker's anatomy, the 19 articulatory
//! parameters with their ranges, the vocal tract shapes (vowels and consonants) and the glottis
//! shapes. File format and parameter set (C) Peter Birkholz, VocalTractLab, GPL-3.0-or-later.

use crate::xml::{self, Node};

pub const NUM_PARAMS: usize = 19;
/// Articulatory parameters in VTL order: hyoid, jaw, lips, velum, tongue body / tip / blade /
/// root and the elevation of the tongue sides.
pub const PARAM_NAMES: [&str; NUM_PARAMS] = [
    "HX", "HY", "JX", "JA", "LP", "LD", "VS", "VO", "TCX", "TCY", "TTX", "TTY", "TBX", "TBY", "TRX", "TRY", "TS1", "TS2", "TS3",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub description: String,
    pub unit: String,
    pub min: f64,
    pub max: f64,
    pub neutral: f64,
}

/// A vocal tract shape: a name and one value per articulatory parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub name: String,
    pub params: [f64; NUM_PARAMS],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PalateRib {
    pub x: f64,
    pub z: f64,
    pub teeth_height: f64,
    pub teeth_width_top: f64,
    pub teeth_width_bottom: f64,
    pub palate_height: f64,
    pub palate_angle_deg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JawRib {
    pub x: f64,
    pub z: f64,
    pub teeth_height: f64,
    pub teeth_width_top: f64,
    pub teeth_width_bottom: f64,
    pub jaw_height: f64,
    pub jaw_angle_deg: f64,
}

/// Speaker-specific anatomy that is independent of the articulation (all lengths in cm).
#[derive(Debug, Clone, PartialEq)]
pub struct Anatomy {
    pub palate: Vec<PalateRib>,
    pub jaw_fulcrum: [f64; 2],
    pub jaw_rest_pos: [f64; 2],
    pub tooth_root_length: f64,
    pub jaw: Vec<JawRib>,
    pub tongue_tip_radius: f64,
    pub tongue_center_radius_x: f64,
    pub tongue_center_radius_y: f64,
    pub automatic_tongue_root_calc: bool,
    pub trx_slope: f64,
    pub trx_intercept: f64,
    pub try_slope: f64,
    pub try_intercept: f64,
    pub lips_width: f64,
    pub uvula_width: f64,
    pub uvula_height: f64,
    pub uvula_depth: f64,
    pub max_nasal_port_area: f64,
    pub velum_low: Vec<[f64; 2]>,
    pub velum_mid: Vec<[f64; 2]>,
    pub velum_high: Vec<[f64; 2]>,
    pub pharynx_fulcrum: [f64; 2],
    pub pharynx_rotation_angle_deg: f64,
    pub pharynx_top_rib_y: f64,
    pub pharynx_upper_depth: f64,
    pub pharynx_lower_depth: f64,
    pub pharynx_back_width: f64,
    pub larynx_upper_depth: f64,
    pub larynx_lower_depth: f64,
    pub epiglottis_width: f64,
    pub epiglottis_height: f64,
    pub epiglottis_depth: f64,
    pub epiglottis_angle_deg: f64,
    pub larynx_narrow: Vec<[f64; 2]>,
    pub larynx_wide: Vec<[f64; 2]>,
    pub piriform_fossa_length: f64,
    pub piriform_fossa_volume: f64,
    pub subglottal_cavity_length: f64,
    pub nasal_cavity_length: f64,
    pub positive_velocity_factor: [f64; NUM_PARAMS],
    pub negative_velocity_factor: [f64; NUM_PARAMS],
}

/// A named set of glottis control values, e.g. "modal", "breathy", "voiceless-plosive".
#[derive(Debug, Clone, PartialEq)]
pub struct GlottisShape {
    pub name: String,
    pub values: Vec<(String, f64)>,
}

impl GlottisShape {
    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Speaker {
    pub params: Vec<Param>,
    pub anatomy: Anatomy,
    pub shapes: Vec<Shape>,
    pub glottis_shapes: Vec<GlottisShape>,
}

fn points(node: &Node, count: usize) -> Result<Vec<[f64; 2]>, String> {
    let s = node.attr("points").ok_or_else(|| format!("<{}> has no 'points'", node.name))?;
    let nums: Vec<f64> = s.split_whitespace().map(|t| t.parse::<f64>().map_err(|_| format!("bad number '{t}' in <{}>", node.name))).collect::<Result<_, _>>()?;
    if nums.len() < 2 * count {
        return Err(format!("<{}> needs {} coordinates but has {}", node.name, 2 * count, nums.len()));
    }
    Ok((0..count).map(|i| [nums[2 * i], nums[2 * i + 1]]).collect())
}

fn read_anatomy(a: &Node) -> Result<Anatomy, String> {
    let n = |node: &Node, k: &str| node.attr_f64(k);

    let palate_node = a.child("palate")?;
    let mut palate = vec![];
    for i in 0.. {
        let Ok(p) = palate_node.child(&format!("p{i}")) else { break };
        palate.push(PalateRib {
            x: n(p, "x")?, z: n(p, "z")?, teeth_height: n(p, "teeth_height")?,
            teeth_width_top: n(p, "top_teeth_width")?, teeth_width_bottom: n(p, "bottom_teeth_width")?,
            palate_height: n(p, "palate_height")?, palate_angle_deg: n(p, "palate_angle_deg")?,
        });
    }
    let jaw_node = a.child("jaw")?;
    let mut jaw = vec![];
    for i in 0.. {
        let Ok(p) = jaw_node.child(&format!("p{i}")) else { break };
        jaw.push(JawRib {
            x: n(p, "x")?, z: n(p, "z")?, teeth_height: n(p, "teeth_height")?,
            teeth_width_top: n(p, "top_teeth_width")?, teeth_width_bottom: n(p, "bottom_teeth_width")?,
            jaw_height: n(p, "jaw_height")?, jaw_angle_deg: n(p, "jaw_angle_deg")?,
        });
    }
    let tongue = a.child("tongue")?;
    let (tip, body, root) = (tongue.child("tip")?, tongue.child("body")?, tongue.child("root")?);
    let velum = a.child("velum")?;
    let pharynx = a.child("pharynx")?;
    let larynx = a.child("larynx")?;

    let mut pos_vel = [1.0; NUM_PARAMS];
    let mut neg_vel = [1.0; NUM_PARAMS];
    for p in a.children_named("param") {
        let i = n(p, "index")? as usize;
        if i >= NUM_PARAMS { return Err(format!("parameter index {i} is out of range")); }
        pos_vel[i] = n(p, "positive_velocity_factor")?;
        neg_vel[i] = n(p, "negative_velocity_factor")?;
    }

    Ok(Anatomy {
        palate,
        jaw_fulcrum: [n(jaw_node, "fulcrum_x")?, n(jaw_node, "fulcrum_y")?],
        jaw_rest_pos: [n(jaw_node, "rest_pos_x")?, n(jaw_node, "rest_pos_y")?],
        tooth_root_length: n(jaw_node, "tooth_root_length")?,
        jaw,
        tongue_tip_radius: n(tip, "radius")?,
        tongue_center_radius_x: n(body, "radius_x")?,
        tongue_center_radius_y: n(body, "radius_y")?,
        automatic_tongue_root_calc: n(root, "automatic_calc")? != 0.0,
        trx_slope: n(root, "trx_slope")?, trx_intercept: n(root, "trx_intercept")?,
        try_slope: n(root, "try_slope")?, try_intercept: n(root, "try_intercept")?,
        lips_width: n(a.child("lips")?, "width")?,
        uvula_width: n(velum, "uvula_width")?, uvula_height: n(velum, "uvula_height")?, uvula_depth: n(velum, "uvula_depth")?,
        max_nasal_port_area: n(velum, "max_nasal_port_area")?,
        velum_low: points(velum.child("low")?, 5)?,
        velum_mid: points(velum.child("mid")?, 5)?,
        velum_high: points(velum.child("high")?, 5)?,
        pharynx_fulcrum: [n(pharynx, "fulcrum_x")?, n(pharynx, "fulcrum_y")?],
        pharynx_rotation_angle_deg: n(pharynx, "rotation_angle_deg")?,
        pharynx_top_rib_y: n(pharynx, "top_rib_y")?,
        pharynx_upper_depth: n(pharynx, "upper_depth")?, pharynx_lower_depth: n(pharynx, "lower_depth")?,
        pharynx_back_width: n(pharynx, "back_side_width")?,
        larynx_upper_depth: n(larynx, "upper_depth")?, larynx_lower_depth: n(larynx, "lower_depth")?,
        epiglottis_width: n(larynx, "epiglottis_width")?, epiglottis_height: n(larynx, "epiglottis_height")?,
        epiglottis_depth: n(larynx, "epiglottis_depth")?, epiglottis_angle_deg: n(larynx, "epiglottis_angle_deg")?,
        larynx_narrow: points(larynx.child("narrow")?, 8)?,
        larynx_wide: points(larynx.child("wide")?, 8)?,
        piriform_fossa_length: n(a.child("piriform_fossa")?, "length")?,
        piriform_fossa_volume: n(a.child("piriform_fossa")?, "volume")?,
        subglottal_cavity_length: n(a.child("subglottal_cavity")?, "length")?,
        nasal_cavity_length: n(a.child("nasal_cavity")?, "length")?,
        positive_velocity_factor: pos_vel,
        negative_velocity_factor: neg_vel,
    })
}

impl Speaker {
    pub fn from_xml(text: &str) -> Result<Speaker, String> {
        let root = xml::parse(text)?;
        if root.name != "speaker" { return Err(format!("root element is <{}>, expected <speaker>", root.name)); }
        let vt = root.child("vocal_tract_model")?;
        let anatomy_node = vt.child("anatomy")?;
        let anatomy = read_anatomy(anatomy_node)?;

        // Parameter definitions, in index order.
        let mut params: Vec<Option<Param>> = vec![None; NUM_PARAMS];
        for p in anatomy_node.children_named("param") {
            let i = p.attr_f64("index")? as usize;
            params[i] = Some(Param {
                name: p.attr("name").unwrap_or("").to_string(),
                description: p.attr("description").unwrap_or("").to_string(),
                unit: p.attr("unit").unwrap_or("").to_string(),
                min: p.attr_f64("min")?, max: p.attr_f64("max")?, neutral: p.attr_f64("neutral")?,
            });
        }
        let params: Vec<Param> = params.into_iter().enumerate()
            .map(|(i, p)| p.ok_or_else(|| format!("parameter {} ({}) is not defined", i, PARAM_NAMES[i])))
            .collect::<Result<_, _>>()?;
        for (i, p) in params.iter().enumerate() {
            if p.name != PARAM_NAMES[i] { return Err(format!("parameter {} is '{}', expected '{}'", i, p.name, PARAM_NAMES[i])); }
        }

        // Vocal tract shapes: unnamed parameters keep their neutral value.
        let mut shapes = vec![];
        for s in vt.child("shapes")?.children_named("shape") {
            let mut values = [0.0; NUM_PARAMS];
            for (i, p) in params.iter().enumerate() { values[i] = p.neutral; }
            for p in s.children_named("param") {
                let name = p.attr("name").ok_or("a shape <param> has no name")?;
                let i = PARAM_NAMES.iter().position(|n| *n == name).ok_or_else(|| format!("unknown shape parameter '{name}'"))?;
                values[i] = p.attr_f64("value")?;
            }
            shapes.push(Shape { name: s.attr("name").unwrap_or("").to_string(), params: values });
        }

        // Glottis shapes of the selected geometric glottis model (first one of that type).
        let mut glottis_shapes = vec![];
        if let Ok(models) = root.child("glottis_models") {
            if let Some(m) = models.children_named("glottis_model").find(|m| m.attr("type") == Some("Geometric glottis")) {
                if let Ok(sh) = m.child("shapes") {
                    for s in sh.children_named("shape") {
                        let values = s.children_named("control_param")
                            .filter_map(|c| Some((c.attr("name")?.to_string(), c.attr_f64("value").ok()?)))
                            .collect();
                        glottis_shapes.push(GlottisShape { name: s.attr("name").unwrap_or("").to_string(), values });
                    }
                }
            }
        }
        Ok(Speaker { params, anatomy, shapes, glottis_shapes })
    }

    pub fn shape(&self, name: &str) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.name == name)
    }

    pub fn glottis_shape(&self, name: &str) -> Option<&GlottisShape> {
        self.glottis_shapes.iter().find(|s| s.name == name)
    }

    /// The neutral position of every parameter.
    pub fn neutral_params(&self) -> [f64; NUM_PARAMS] {
        let mut v = [0.0; NUM_PARAMS];
        for (i, p) in self.params.iter().enumerate() { v[i] = p.neutral; }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JD3: &str = include_str!("../../../resources/JD3.speaker");

    fn jd3() -> Speaker {
        Speaker::from_xml(JD3).expect("JD3.speaker must load")
    }

    #[test]
    fn loads_the_bundled_speaker_with_all_parameters() {
        let s = jd3();
        assert_eq!(s.params.len(), NUM_PARAMS);
        assert_eq!(s.params[0].name, "HX");
        assert_eq!(s.params[18].name, "TS3");
        let lp = &s.params[4];
        assert_eq!((lp.min, lp.max, lp.neutral), (-1.0, 1.0, -0.07));
        for p in &s.params { assert!(p.min <= p.neutral && p.neutral <= p.max, "{}", p.name); }
    }

    #[test]
    fn loads_anatomy() {
        let a = jd3().anatomy;
        assert_eq!(a.palate.len(), 9);
        assert_eq!(a.jaw.len(), 9);
        assert_eq!(a.velum_low.len(), 5);
        assert_eq!(a.larynx_narrow.len(), 8);
        assert!((a.palate[0].x - 0.2).abs() < 1e-12 && (a.palate[0].z + 2.3).abs() < 1e-12);
        assert!(a.tongue_tip_radius > 0.0 && a.lips_width > 0.0 && a.nasal_cavity_length > 5.0);
        assert!(a.positive_velocity_factor.iter().all(|v| *v > 0.0));
    }

    #[test]
    fn vowel_and_consonant_shapes_are_present() {
        let s = jd3();
        assert_eq!(s.shapes.len(), 69, "vocal tract shapes (the 94 <shape> tags in the file include glottis shapes)");
        for v in ["a", "e", "i", "o", "u"] { assert!(s.shape(v).is_some(), "vowel {v}"); }
        // Consonants are defined in the context of the three corner vowels a, i, u.
        for c in ["tt-alveolar-closure", "tb-velar-closure", "ll-labial-closure", "tt-alveolar-fricative",
                  "tt-postalveolar-fricative", "tb-palatal-fricative"] {
            for v in ["a", "i", "u"] { assert!(s.shape(&format!("{c}({v})")).is_some(), "{c}({v})"); }
        }
        let a = s.shape("a").unwrap();
        assert!((a.params[0] - 0.1667).abs() < 1e-9 && (a.params[3] + 4.1498).abs() < 1e-9);
        // The velum is closed for oral vowels (VO at its lower limit region).
        assert!(a.params[7] <= 0.0);
    }

    #[test]
    fn velar_closure_differs_between_vowel_contexts() {
        let s = jd3();
        let (ka, ki) = (s.shape("tb-velar-closure(a)").unwrap(), s.shape("tb-velar-closure(i)").unwrap());
        let tcx = PARAM_NAMES.iter().position(|n| *n == "TCX").unwrap();
        assert!((ka.params[tcx] - ki.params[tcx]).abs() > 0.5, "tongue body should be more fronted before /i/");
    }

    #[test]
    fn loads_glottis_shapes() {
        let s = jd3();
        for n in ["modal", "breathy", "pressed", "voiceless-plosive", "voiceless-fricative", "whisper"] {
            assert!(s.glottis_shape(n).is_some(), "glottis shape {n}");
        }
        let modal = s.glottis_shape("modal").unwrap();
        assert!(modal.get("XB").is_some() && modal.get("AS").is_some());
        let plosive = s.glottis_shape("voiceless-plosive").unwrap();
        // A voiceless plosive opens the glottis much wider than modal voice.
        assert!(plosive.get("XB").unwrap() > modal.get("XB").unwrap() + 0.05);
    }

    #[test]
    fn rejects_broken_files() {
        assert!(Speaker::from_xml("<speaker/>").is_err());
        assert!(Speaker::from_xml("<other/>").is_err());
        assert!(Speaker::from_xml(&JD3.replace("name=\"TBX\"", "name=\"XXX\"")).is_err());
    }
}
