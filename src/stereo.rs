//! Stereo layout, color composition and validated portable preferences.
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Auto,
    Mono,
    FullSbs,
    HalfSbs,
    FullOu,
    HalfOu,
    Mvc,
}
impl Layout {
    pub const ALL: [Self; 7] = [
        Self::Auto,
        Self::Mono,
        Self::FullSbs,
        Self::HalfSbs,
        Self::FullOu,
        Self::HalfOu,
        Self::Mvc,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automático / metadatos",
            Self::Mono => "2D / cuadro único",
            Self::FullSbs => "Full SBS",
            Self::HalfSbs => "Half SBS",
            Self::FullOu => "Full OU",
            Self::HalfOu => "HOU / Half OU",
            Self::Mvc => "MVC / Blu-ray 3D",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Mono => "mono",
            Self::FullSbs => "fsbs",
            Self::HalfSbs => "hsbs",
            Self::FullOu => "fou",
            Self::HalfOu => "hou",
            Self::Mvc => "mvc",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.code() == s)
    }
    pub fn horizontal(self) -> bool {
        matches!(self, Self::FullSbs | Self::HalfSbs | Self::Mvc)
    }
    pub fn packed(self) -> bool {
        self.horizontal() || matches!(self, Self::FullOu | Self::HalfOu)
    }
    pub fn ffmpeg(self, swap: bool) -> Option<&'static str> {
        match (self, swap) {
            (Self::FullSbs | Self::Mvc, false) => Some("sbsl"),
            (Self::FullSbs | Self::Mvc, true) => Some("sbsr"),
            (Self::HalfSbs, false) => Some("sbs2l"),
            (Self::HalfSbs, true) => Some("sbs2r"),
            (Self::FullOu, false) => Some("abl"),
            (Self::FullOu, true) => Some("abr"),
            (Self::HalfOu, false) => Some("ab2l"),
            (Self::HalfOu, true) => Some("ab2r"),
            _ => None,
        }
    }
    pub fn eye(self, w: u32, h: u32) -> Result<(u32, u32, f64), String> {
        if w == 0 || h == 0 {
            return Err("El vídeo todavía no tiene dimensiones válidas".into());
        }
        let (ew, eh, aspect) = match self {
            Self::FullSbs | Self::Mvc => (w / 2, h, w as f64 / (2.0 * h as f64)),
            Self::HalfSbs => (w / 2, h, w as f64 / h as f64),
            Self::FullOu => (w, h / 2, 2.0 * w as f64 / h as f64),
            Self::HalfOu => (w, h / 2, w as f64 / h as f64),
            _ => (w, h, w as f64 / h as f64),
        };
        if (self.horizontal() && !w.is_multiple_of(2))
            || (matches!(self, Self::FullOu | Self::HalfOu) && !h.is_multiple_of(2))
        {
            return Err("El cuadro 3D debe dividirse en dos mitades enteras".into());
        }
        Ok((ew, eh, aspect))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Output {
    Original,
    #[default]
    Left,
    Right,
    Anaglyph,
}
impl Output {
    pub const ALL: [Self; 4] = [Self::Original, Self::Left, Self::Right, Self::Anaglyph];
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Original / las dos vistas",
            Self::Left => "2D · ojo izquierdo",
            Self::Right => "2D · ojo derecho",
            Self::Anaglyph => "Anaglifo",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Glasses {
    #[default]
    RedCyan,
    GreenMagenta,
    AmberBlue,
    RedBlue,
    RedGreen,
}
impl Glasses {
    pub const ALL: [Self; 5] = [
        Self::RedCyan,
        Self::GreenMagenta,
        Self::AmberBlue,
        Self::RedBlue,
        Self::RedGreen,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::RedCyan => "Rojo / cian",
            Self::GreenMagenta => "Verde / magenta",
            Self::AmberBlue => "Ámbar / azul",
            Self::RedBlue => "Rojo / azul (monocromo)",
            Self::RedGreen => "Rojo / verde (monocromo)",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Algorithm {
    #[default]
    Dubois,
    Half,
    Mono,
    Color,
}
impl Algorithm {
    pub const ALL: [Self; 4] = [Self::Dubois, Self::Half, Self::Mono, Self::Color];
    pub fn label(self) -> &'static str {
        match self {
            Self::Dubois => "Dubois / color optimizado",
            Self::Half => "Medio color",
            Self::Mono => "Monocromo",
            Self::Color => "Color directo",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    #[default]
    Balanced,
    High,
    Compatibility,
}
impl Quality {
    pub const ALL: [Self; 3] = [Self::Balanced, Self::High, Self::Compatibility];
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "Equilibrado · GPU",
            Self::High => "Máxima calidad · GPU",
            Self::Compatibility => "Compatibilidad · filtro CPU",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub layout: Layout,
    pub output: Output,
    pub glasses: Glasses,
    pub algorithm: Algorithm,
    pub quality: Quality,
    pub swap: bool,
    pub aspect: Option<f64>,
    pub convergence: f32,
    pub left_gain: f32,
    pub right_gain: f32,
    pub crosstalk: f32,
    pub sub_pos: i32,
    pub ass_mode: crate::ass3d::Mode,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            layout: Layout::Auto,
            output: Output::Left,
            glasses: Glasses::RedCyan,
            algorithm: Algorithm::Dubois,
            quality: Quality::Balanced,
            swap: false,
            aspect: None,
            convergence: 0.0,
            left_gain: 1.0,
            right_gain: 1.0,
            crosstalk: 0.0,
            sub_pos: 100,
            ass_mode: crate::ass3d::Mode::Auto,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        for (v, min, max) in [
            (self.convergence, -0.03, 0.03),
            (self.left_gain, 0.5, 1.5),
            (self.right_gain, 0.5, 1.5),
            (self.crosstalk, 0.0, 0.15),
        ] {
            if !v.is_finite() || v < min || v > max {
                return Err("Ajuste 3D fuera de rango".into());
            }
        }
        if !(50..=110).contains(&self.sub_pos)
            || self
                .aspect
                .is_some_and(|v| !v.is_finite() || !(0.1..=10.0).contains(&v))
        {
            return Err("Aspecto o posición de subtítulos fuera de rango".into());
        }
        Ok(())
    }
    pub fn encode(&self) -> String {
        format!(
            "version=1\nlayout={}\noutput={}\nglasses={}\nalgorithm={}\nquality={}\nswap={}\naspect={}\nconvergence={}\nleft_gain={}\nright_gain={}\ncrosstalk={}\nsub_pos={}\nass_mode={}\n",
            self.layout.code(),
            self.output as u8,
            self.glasses as u8,
            self.algorithm as u8,
            self.quality as u8,
            self.swap,
            self.aspect.unwrap_or(0.0),
            self.convergence,
            self.left_gain,
            self.right_gain,
            self.crosstalk,
            self.sub_pos,
            self.ass_mode as u8,
        )
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > 4096 {
            return Err("Perfil demasiado grande".into());
        }
        let mut s = Self::default();
        let mut version = false;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let bad = || format!("Valor de perfil inválido: {k}");
            match k {
                "version" => {
                    if v != "1" {
                        return Err("Versión de perfil no admitida".into());
                    }
                    version = true;
                }
                "layout" => s.layout = Layout::parse(v).ok_or_else(bad)?,
                "output" => {
                    s.output = *Output::ALL
                        .get(v.parse::<usize>().map_err(|_| bad())?)
                        .ok_or_else(bad)?
                }
                "glasses" => {
                    s.glasses = *Glasses::ALL
                        .get(v.parse::<usize>().map_err(|_| bad())?)
                        .ok_or_else(bad)?
                }
                "algorithm" => {
                    s.algorithm = *Algorithm::ALL
                        .get(v.parse::<usize>().map_err(|_| bad())?)
                        .ok_or_else(bad)?
                }
                "quality" => {
                    s.quality = *Quality::ALL
                        .get(v.parse::<usize>().map_err(|_| bad())?)
                        .ok_or_else(bad)?
                }
                "swap" => s.swap = v.parse().map_err(|_| bad())?,
                "aspect" => {
                    let n = v.parse::<f64>().map_err(|_| bad())?;
                    s.aspect = if n == 0.0 { None } else { Some(n) };
                }
                "convergence" => s.convergence = v.parse().map_err(|_| bad())?,
                "left_gain" => s.left_gain = v.parse().map_err(|_| bad())?,
                "right_gain" => s.right_gain = v.parse().map_err(|_| bad())?,
                "crosstalk" => s.crosstalk = v.parse().map_err(|_| bad())?,
                "sub_pos" => s.sub_pos = v.parse().map_err(|_| bad())?,
                "ass_mode" => {
                    s.ass_mode = *crate::ass3d::Mode::ALL
                        .get(v.parse::<usize>().map_err(|_| bad())?)
                        .ok_or_else(bad)?
                }
                _ => {}
            }
        }
        if !version {
            return Err("El perfil no contiene versión".into());
        }
        s.validate()?;
        Ok(s)
    }
}

/// Metadata is authoritative when unambiguous; generic 'SBS' names are not guessed.
pub fn detect(metadata: &str, name: &str) -> Layout {
    match metadata {
        "sbs2l" | "sbs2r" => return Layout::HalfSbs,
        "ab2l" | "ab2r" => return Layout::HalfOu,
        _ => {}
    }
    let name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    let tokens = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let single = |value: &str| tokens.contains(&value);
    let pair = |a: &str, b: &str| tokens.windows(2).any(|p| p == [a, b]);
    if single("fullsbs") || pair("full", "sbs") {
        Layout::FullSbs
    } else if single("hsbs") || pair("half", "sbs") {
        Layout::HalfSbs
    } else if single("fullou") || single("fulltab") || pair("full", "ou") || pair("full", "tab") {
        Layout::FullOu
    } else if single("hou") || pair("half", "ou") || pair("half", "tab") {
        Layout::HalfOu
    } else if single("mvc") {
        Layout::Mvc
    } else {
        Layout::Mono
    }
}

/// A hint alone cannot distinguish full from half packing.
pub fn stereo_hint(metadata: &str, name: &str) -> bool {
    matches!(
        metadata,
        "sbsl" | "sbsr" | "abl" | "abr" | "sbs2l" | "sbs2r" | "ab2l" | "ab2r"
    ) || name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|s| matches!(s, "sbs" | "ou" | "tab" | "3d" | "mvc"))
}

/// Rows are RGB output; columns are left RGB followed by right RGB.
/// Dubois preset coefficients follow FFmpeg's GPL stereo3d reference, /65536.
pub fn matrix(g: Glasses, a: Algorithm) -> [[f32; 6]; 3] {
    if a == Algorithm::Dubois {
        let ints = match g {
            Glasses::RedCyan => Some([
                [29884, 32768, 11534, -2818, -5767, -131],
                [-2621, -2490, -1049, 24773, 48103, -1180],
                [-983, -1376, -328, -4719, -7406, 80347],
            ]),
            Glasses::GreenMagenta => Some([
                [-4063, -10354, -2556, 34669, 46203, 1573],
                [18612, 43778, 9372, -1049, -983, -4260],
                [-983, -1769, 1376, 590, 4915, 61407],
            ]),
            Glasses::AmberBlue => Some([
                [69599, -13435, 19595, -1048, -8061, -1114],
                [-1704, 59507, 4456, 393, 4063, -1114],
                [-2490, -11338, 1442, 6160, 12124, 59703],
            ]),
            _ => None,
        };
        if let Some(m) = ints {
            return m.map(|row| row.map(|v| v as f32 / 65536.0));
        }
    }
    let mut m = [[0.0; 6]; 3];
    let mono = a == Algorithm::Mono || matches!(g, Glasses::RedBlue | Glasses::RedGreen);
    let left = match g {
        Glasses::RedCyan | Glasses::RedBlue | Glasses::RedGreen => [true, false, false],
        Glasses::GreenMagenta => [false, true, false],
        Glasses::AmberBlue => [true, true, false],
    };
    for channel in 0..3 {
        if (g == Glasses::RedBlue && channel == 1) || (g == Glasses::RedGreen && channel == 2) {
            continue;
        }
        let eye = if left[channel] { 0 } else { 3 };
        let half_mono = if g == Glasses::AmberBlue {
            !left[channel]
        } else {
            left[channel]
        };
        if mono || (a == Algorithm::Half && half_mono) {
            m[channel][eye..eye + 3].copy_from_slice(&[0.299, 0.587, 0.114]);
        } else {
            m[channel][eye + channel] = 1.0;
        }
    }
    m
}

pub fn cpu_output(g: Glasses, a: Algorithm) -> &'static str {
    match (g, a) {
        (Glasses::RedBlue, _) => "arbg",
        (Glasses::RedGreen, _) => "argg",
        (Glasses::RedCyan, Algorithm::Dubois) => "arcd",
        (Glasses::RedCyan, Algorithm::Half) => "arch",
        (Glasses::RedCyan, Algorithm::Mono) => "arcg",
        (Glasses::RedCyan, Algorithm::Color) => "arcc",
        (Glasses::GreenMagenta, Algorithm::Dubois) => "agmd",
        (Glasses::GreenMagenta, Algorithm::Half) => "agmh",
        (Glasses::GreenMagenta, Algorithm::Mono) => "agmg",
        (Glasses::GreenMagenta, Algorithm::Color) => "agmc",
        (Glasses::AmberBlue, Algorithm::Dubois) => "aybd",
        (Glasses::AmberBlue, Algorithm::Half) => "aybh",
        (Glasses::AmberBlue, Algorithm::Mono) => "aybg",
        (Glasses::AmberBlue, Algorithm::Color) => "aybc",
    }
}

pub fn shader(s: &Settings, layout: Layout, srgb: bool) -> Result<String, String> {
    s.validate()?;
    if !layout.packed() {
        return Err("Selecciona el formato 3D antes de activar el anaglifo".into());
    }
    let mut out =
        String::from("//!HOOK MAIN\n//!BIND HOOKED\n//!DESC Enigmax anaglyph linear RGB\n");
    if layout.horizontal() {
        out.push_str("//!WIDTH HOOKED.w 2 /\n");
    } else {
        out.push_str("//!HEIGHT HOOKED.h 2 /\n");
    }
    out.push_str("vec3 lin(vec3 c) { c=max(c,vec3(0.0)); ");
    if srgb {
        out.push_str("return mix(pow((c+0.055)/1.055,vec3(2.4)),c/12.92,lessThanEqual(c,vec3(0.04045))); }\nvec3 enc(vec3 c) {c=clamp(c,0.0,1.0);return mix(1.055*pow(c,vec3(1.0/2.4))-0.055,12.92*c,lessThanEqual(c,vec3(0.0031308)));}\n");
    } else {
        out.push_str("return pow(c,vec3(2.4));}\nvec3 enc(vec3 c){return pow(clamp(c,0.0,1.0),vec3(1.0/2.4));}\n");
    }
    out.push_str("vec4 hook(){vec2 p=HOOKED_pos;vec2 a=p;vec2 b=p;\n");
    let shift = s.convergence;
    if layout.horizontal() {
        writeln!(out,"a.x=clamp((p.x+{shift:.7})*0.5,0.5*HOOKED_pt.x,0.5-0.5*HOOKED_pt.x);b.x=clamp((p.x-{shift:.7})*0.5+0.5,0.5+0.5*HOOKED_pt.x,1.0-0.5*HOOKED_pt.x);").unwrap();
    } else {
        writeln!(out,"a.x=clamp(p.x+{shift:.7},0.5*HOOKED_pt.x,1.0-0.5*HOOKED_pt.x);b.x=clamp(p.x-{shift:.7},0.5*HOOKED_pt.x,1.0-0.5*HOOKED_pt.x);a.y=p.y*0.5;b.y=p.y*0.5+0.5;").unwrap();
    }
    if s.swap {
        out.push_str("vec2 t=a;a=b;b=t;\n");
    }
    writeln!(out,"vec3 l=lin(HOOKED_tex(a).rgb)*{:.7};vec3 r=lin(HOOKED_tex(b).rgb)*{:.7};vec3 ol=l;l=max(l-r*{:.7},0.0);r=max(r-ol*{:.7},0.0);",s.left_gain,s.right_gain,s.crosstalk,s.crosstalk).unwrap();
    let m = matrix(s.glasses, s.algorithm);
    out.push_str("vec3 v;\n");
    for (i, channel) in ["r", "g", "b"].iter().enumerate() {
        let a = m[i];
        writeln!(
            out,
            "v.{channel}=dot(l,vec3({:.8},{:.8},{:.8}))+dot(r,vec3({:.8},{:.8},{:.8}));",
            a[0], a[1], a[2], a[3], a[4], a[5]
        )
        .unwrap();
    }
    out.push_str("return vec4(enc(v),1.0);}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layouts_and_cropped_aspect() {
        for l in [
            Layout::FullSbs,
            Layout::HalfSbs,
            Layout::FullOu,
            Layout::HalfOu,
        ] {
            let (w, h) = match l {
                Layout::FullSbs => (3840, 1080),
                Layout::FullOu => (1920, 2160),
                _ => (1920, 1080),
            };
            assert!((l.eye(w, h).unwrap().2 - 16.0 / 9.0).abs() < 1e-8);
        }
        assert_eq!(Layout::FullSbs.eye(3840, 800).unwrap().2, 2.4);
        assert!(Layout::HalfSbs.eye(1919, 1080).is_err());
    }
    #[test]
    fn validated_profile_roundtrip() {
        let s = Settings {
            layout: Layout::HalfOu,
            output: Output::Anaglyph,
            swap: true,
            convergence: 0.01,
            ..Settings::default()
        };
        assert_eq!(s, Settings::decode(&s.encode()).unwrap());
        for t in [
            "version=1\nconvergence=NaN",
            "version=1\nglasses=99",
            "version=2",
            "version=1\nsub_pos=500",
        ] {
            assert!(Settings::decode(t).is_err());
        }
    }
    #[test]
    fn all_modes_generate_finite_matrices_and_shader() {
        for g in Glasses::ALL {
            for a in Algorithm::ALL {
                let s = Settings {
                    glasses: g,
                    algorithm: a,
                    ..Settings::default()
                };
                assert!(matrix(g, a).iter().flatten().all(|v| v.is_finite()));
                let src = shader(&s, Layout::FullSbs, true).unwrap();
                assert!(src.contains("//!WIDTH HOOKED.w 2 /"));
                assert!(!src.contains("NaN"));
            }
        }
    }
    #[test]
    fn direct_color_separates_correct_eyes() {
        let m = matrix(Glasses::RedCyan, Algorithm::Color);
        assert_eq!(m[0], [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(m[1][4], 1.0);
        assert_eq!(m[2][5], 1.0);
        let ab = matrix(Glasses::AmberBlue, Algorithm::Color);
        assert_eq!(ab[0][0], 1.0);
        assert_eq!(ab[1][1], 1.0);
        assert_eq!(ab[2][5], 1.0);
    }
    #[test]
    fn ambiguous_names_are_not_guessed() {
        assert_eq!(detect("sbsl", "pelicula.mkv"), Layout::Mono);
        assert_eq!(detect("", "pelicula-SBS.mkv"), Layout::Mono);
        assert_eq!(detect("", "pelicula-Full-SBS.mkv"), Layout::FullSbs);
        assert_eq!(detect("", "pelicula.HSBS.mp4"), Layout::HalfSbs);
        assert_eq!(detect("", "House.mkv"), Layout::Mono);
        assert_eq!(detect("", "C:/peliculas-HOU/Normal.mkv"), Layout::Mono);
        assert_eq!(detect("", "Film (Full SBS).mkv"), Layout::FullSbs);
        assert_eq!(detect("", "Film.MVC.mkv"), Layout::Mvc);
        assert!(stereo_hint("sbsl", "Film.mkv"));
        assert!(stereo_hint("", "Film (3D).mkv"));
        assert!(!stereo_hint("", "Somebody.mkv"));
    }
}
