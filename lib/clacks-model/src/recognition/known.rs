// Recognition variants cover every explicit label in the pinned ADIF data.
use super::mode::ModeIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KnownBand {
    Meters2190,
    Meters630,
    Meters560,
    Meters160,
    Meters80,
    Meters60,
    Meters40,
    Meters30,
    Meters20,
    Meters17,
    Meters15,
    Meters12,
    Meters10,
    Meters8,
    Meters6,
    Meters5,
    Meters4,
    Meters2,
    Meters1_25,
    Centimeters70,
    Centimeters33,
    Centimeters23,
    Centimeters13,
    Centimeters9,
    Centimeters6,
    Centimeters3,
    Centimeters1_25,
    Millimeters6,
    Millimeters4,
    Millimeters2_5,
    Millimeters2,
    Millimeters1,
    Submillimeter,
}

impl KnownBand {
    pub fn label(self) -> &'static str {
        match self {
            Self::Meters2190 => "2190m",
            Self::Meters630 => "630m",
            Self::Meters560 => "560m",
            Self::Meters160 => "160m",
            Self::Meters80 => "80m",
            Self::Meters60 => "60m",
            Self::Meters40 => "40m",
            Self::Meters30 => "30m",
            Self::Meters20 => "20m",
            Self::Meters17 => "17m",
            Self::Meters15 => "15m",
            Self::Meters12 => "12m",
            Self::Meters10 => "10m",
            Self::Meters8 => "8m",
            Self::Meters6 => "6m",
            Self::Meters5 => "5m",
            Self::Meters4 => "4m",
            Self::Meters2 => "2m",
            Self::Meters1_25 => "1.25m",
            Self::Centimeters70 => "70cm",
            Self::Centimeters33 => "33cm",
            Self::Centimeters23 => "23cm",
            Self::Centimeters13 => "13cm",
            Self::Centimeters9 => "9cm",
            Self::Centimeters6 => "6cm",
            Self::Centimeters3 => "3cm",
            Self::Centimeters1_25 => "1.25cm",
            Self::Millimeters6 => "6mm",
            Self::Millimeters4 => "4mm",
            Self::Millimeters2_5 => "2.5mm",
            Self::Millimeters2 => "2mm",
            Self::Millimeters1 => "1mm",
            Self::Submillimeter => "submm",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "2190m" => Some(Self::Meters2190),
            "630m" => Some(Self::Meters630),
            "560m" => Some(Self::Meters560),
            "160m" => Some(Self::Meters160),
            "80m" => Some(Self::Meters80),
            "60m" => Some(Self::Meters60),
            "40m" => Some(Self::Meters40),
            "30m" => Some(Self::Meters30),
            "20m" => Some(Self::Meters20),
            "17m" => Some(Self::Meters17),
            "15m" => Some(Self::Meters15),
            "12m" => Some(Self::Meters12),
            "10m" => Some(Self::Meters10),
            "8m" => Some(Self::Meters8),
            "6m" => Some(Self::Meters6),
            "5m" => Some(Self::Meters5),
            "4m" => Some(Self::Meters4),
            "2m" => Some(Self::Meters2),
            "1.25m" => Some(Self::Meters1_25),
            "70cm" => Some(Self::Centimeters70),
            "33cm" => Some(Self::Centimeters33),
            "23cm" => Some(Self::Centimeters23),
            "13cm" => Some(Self::Centimeters13),
            "9cm" => Some(Self::Centimeters9),
            "6cm" => Some(Self::Centimeters6),
            "3cm" => Some(Self::Centimeters3),
            "1.25cm" => Some(Self::Centimeters1_25),
            "6mm" => Some(Self::Millimeters6),
            "4mm" => Some(Self::Millimeters4),
            "2.5mm" => Some(Self::Millimeters2_5),
            "2mm" => Some(Self::Millimeters2),
            "1mm" => Some(Self::Millimeters1),
            "submm" => Some(Self::Submillimeter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChipSubmode {
    Chip64,
    Chip128,
}

impl ChipSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chip64 => "CHIP64",
            Self::Chip128 => "CHIP128",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "CHIP64" => Some(Self::Chip64),
            "CHIP128" => Some(Self::Chip128),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CwSubmode {
    Pcw,
}

impl CwSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pcw => "PCW",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "PCW" => Some(Self::Pcw),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DigitalvoiceSubmode {
    C4fm,
    Dmr,
    Dstar,
    Freedv,
    M17,
}

impl DigitalvoiceSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::C4fm => "C4FM",
            Self::Dmr => "DMR",
            Self::Dstar => "DSTAR",
            Self::Freedv => "FREEDV",
            Self::M17 => "M17",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "C4FM" => Some(Self::C4fm),
            "DMR" => Some(Self::Dmr),
            "DSTAR" => Some(Self::Dstar),
            "FREEDV" => Some(Self::Freedv),
            "M17" => Some(Self::M17),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DominoSubmode {
    DomM,
    Dom4,
    Dom5,
    Dom8,
    Dom11,
    Dom16,
    Dom22,
    Dom44,
    Dom88,
    Dominoex,
    Dominof,
}

impl DominoSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::DomM => "DOM-M",
            Self::Dom4 => "DOM4",
            Self::Dom5 => "DOM5",
            Self::Dom8 => "DOM8",
            Self::Dom11 => "DOM11",
            Self::Dom16 => "DOM16",
            Self::Dom22 => "DOM22",
            Self::Dom44 => "DOM44",
            Self::Dom88 => "DOM88",
            Self::Dominoex => "DOMINOEX",
            Self::Dominof => "DOMINOF",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "DOM-M" => Some(Self::DomM),
            "DOM4" => Some(Self::Dom4),
            "DOM5" => Some(Self::Dom5),
            "DOM8" => Some(Self::Dom8),
            "DOM11" => Some(Self::Dom11),
            "DOM16" => Some(Self::Dom16),
            "DOM22" => Some(Self::Dom22),
            "DOM44" => Some(Self::Dom44),
            "DOM88" => Some(Self::Dom88),
            "DOMINOEX" => Some(Self::Dominoex),
            "DOMINOF" => Some(Self::Dominof),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DynamicSubmode {
    Freedata,
    VaraHf,
    VaraSatellite,
    VaraFm1200,
    VaraFm9600,
}

impl DynamicSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Freedata => "FREEDATA",
            Self::VaraHf => "VARA HF",
            Self::VaraSatellite => "VARA SATELLITE",
            Self::VaraFm1200 => "VARA FM 1200",
            Self::VaraFm9600 => "VARA FM 9600",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "FREEDATA" => Some(Self::Freedata),
            "VARA HF" => Some(Self::VaraHf),
            "VARA SATELLITE" => Some(Self::VaraSatellite),
            "VARA FM 1200" => Some(Self::VaraFm1200),
            "VARA FM 9600" => Some(Self::VaraFm9600),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FskSubmode {
    ScampFast,
    ScampSlow,
    ScampVslow,
}

impl FskSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ScampFast => "SCAMP_FAST",
            Self::ScampSlow => "SCAMP_SLOW",
            Self::ScampVslow => "SCAMP_VSLOW",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "SCAMP_FAST" => Some(Self::ScampFast),
            "SCAMP_SLOW" => Some(Self::ScampSlow),
            "SCAMP_VSLOW" => Some(Self::ScampVslow),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HellSubmode {
    Fmhell,
    Fskh105,
    Fskh245,
    Fskhell,
    Hell80,
    Hellx5,
    Hellx9,
    Hfsk,
    Pskhell,
    Slowhell,
}

impl HellSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fmhell => "FMHELL",
            Self::Fskh105 => "FSKH105",
            Self::Fskh245 => "FSKH245",
            Self::Fskhell => "FSKHELL",
            Self::Hell80 => "HELL80",
            Self::Hellx5 => "HELLX5",
            Self::Hellx9 => "HELLX9",
            Self::Hfsk => "HFSK",
            Self::Pskhell => "PSKHELL",
            Self::Slowhell => "SLOWHELL",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "FMHELL" => Some(Self::Fmhell),
            "FSKH105" => Some(Self::Fskh105),
            "FSKH245" => Some(Self::Fskh245),
            "FSKHELL" => Some(Self::Fskhell),
            "HELL80" => Some(Self::Hell80),
            "HELLX5" => Some(Self::Hellx5),
            "HELLX9" => Some(Self::Hellx9),
            "HFSK" => Some(Self::Hfsk),
            "PSKHELL" => Some(Self::Pskhell),
            "SLOWHELL" => Some(Self::Slowhell),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IscatSubmode {
    IscatA,
    IscatB,
}

impl IscatSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::IscatA => "ISCAT-A",
            Self::IscatB => "ISCAT-B",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "ISCAT-A" => Some(Self::IscatA),
            "ISCAT-B" => Some(Self::IscatB),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jt4Submode {
    Jt4a,
    Jt4b,
    Jt4c,
    Jt4d,
    Jt4e,
    Jt4f,
    Jt4g,
}

impl Jt4Submode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jt4a => "JT4A",
            Self::Jt4b => "JT4B",
            Self::Jt4c => "JT4C",
            Self::Jt4d => "JT4D",
            Self::Jt4e => "JT4E",
            Self::Jt4f => "JT4F",
            Self::Jt4g => "JT4G",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "JT4A" => Some(Self::Jt4a),
            "JT4B" => Some(Self::Jt4b),
            "JT4C" => Some(Self::Jt4c),
            "JT4D" => Some(Self::Jt4d),
            "JT4E" => Some(Self::Jt4e),
            "JT4F" => Some(Self::Jt4f),
            "JT4G" => Some(Self::Jt4g),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jt9Submode {
    Jt91,
    Jt92,
    Jt95,
    Jt910,
    Jt930,
    Jt9a,
    Jt9b,
    Jt9c,
    Jt9d,
    Jt9e,
    Jt9eFast,
    Jt9f,
    Jt9fFast,
    Jt9g,
    Jt9gFast,
    Jt9h,
    Jt9hFast,
}

impl Jt9Submode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jt91 => "JT9-1",
            Self::Jt92 => "JT9-2",
            Self::Jt95 => "JT9-5",
            Self::Jt910 => "JT9-10",
            Self::Jt930 => "JT9-30",
            Self::Jt9a => "JT9A",
            Self::Jt9b => "JT9B",
            Self::Jt9c => "JT9C",
            Self::Jt9d => "JT9D",
            Self::Jt9e => "JT9E",
            Self::Jt9eFast => "JT9E FAST",
            Self::Jt9f => "JT9F",
            Self::Jt9fFast => "JT9F FAST",
            Self::Jt9g => "JT9G",
            Self::Jt9gFast => "JT9G FAST",
            Self::Jt9h => "JT9H",
            Self::Jt9hFast => "JT9H FAST",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "JT9-1" => Some(Self::Jt91),
            "JT9-2" => Some(Self::Jt92),
            "JT9-5" => Some(Self::Jt95),
            "JT9-10" => Some(Self::Jt910),
            "JT9-30" => Some(Self::Jt930),
            "JT9A" => Some(Self::Jt9a),
            "JT9B" => Some(Self::Jt9b),
            "JT9C" => Some(Self::Jt9c),
            "JT9D" => Some(Self::Jt9d),
            "JT9E" => Some(Self::Jt9e),
            "JT9E FAST" => Some(Self::Jt9eFast),
            "JT9F" => Some(Self::Jt9f),
            "JT9F FAST" => Some(Self::Jt9fFast),
            "JT9G" => Some(Self::Jt9g),
            "JT9G FAST" => Some(Self::Jt9gFast),
            "JT9H" => Some(Self::Jt9h),
            "JT9H FAST" => Some(Self::Jt9hFast),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jt65Submode {
    Jt65a,
    Jt65b,
    Jt65b2,
    Jt65c,
    Jt65c2,
}

impl Jt65Submode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jt65a => "JT65A",
            Self::Jt65b => "JT65B",
            Self::Jt65b2 => "JT65B2",
            Self::Jt65c => "JT65C",
            Self::Jt65c2 => "JT65C2",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "JT65A" => Some(Self::Jt65a),
            "JT65B" => Some(Self::Jt65b),
            "JT65B2" => Some(Self::Jt65b2),
            "JT65C" => Some(Self::Jt65c),
            "JT65C2" => Some(Self::Jt65c2),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MfskSubmode {
    Fsqcall,
    Fst4,
    Fst4w,
    Ft2,
    Ft4,
    Js8,
    Jtms,
    Jtty,
    Mfsk4,
    Mfsk8,
    Mfsk11,
    Mfsk16,
    Mfsk22,
    Mfsk31,
    Mfsk32,
    Mfsk64,
    Mfsk64l,
    Mfsk128,
    Mfsk128l,
    Q65,
}

impl MfskSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fsqcall => "FSQCALL",
            Self::Fst4 => "FST4",
            Self::Fst4w => "FST4W",
            Self::Ft2 => "FT2",
            Self::Ft4 => "FT4",
            Self::Js8 => "JS8",
            Self::Jtms => "JTMS",
            Self::Jtty => "JTTY",
            Self::Mfsk4 => "MFSK4",
            Self::Mfsk8 => "MFSK8",
            Self::Mfsk11 => "MFSK11",
            Self::Mfsk16 => "MFSK16",
            Self::Mfsk22 => "MFSK22",
            Self::Mfsk31 => "MFSK31",
            Self::Mfsk32 => "MFSK32",
            Self::Mfsk64 => "MFSK64",
            Self::Mfsk64l => "MFSK64L",
            Self::Mfsk128 => "MFSK128",
            Self::Mfsk128l => "MFSK128L",
            Self::Q65 => "Q65",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "FSQCALL" => Some(Self::Fsqcall),
            "FST4" => Some(Self::Fst4),
            "FST4W" => Some(Self::Fst4w),
            "FT2" => Some(Self::Ft2),
            "FT4" => Some(Self::Ft4),
            "JS8" => Some(Self::Js8),
            "JTMS" => Some(Self::Jtms),
            "JTTY" => Some(Self::Jtty),
            "MFSK4" => Some(Self::Mfsk4),
            "MFSK8" => Some(Self::Mfsk8),
            "MFSK11" => Some(Self::Mfsk11),
            "MFSK16" => Some(Self::Mfsk16),
            "MFSK22" => Some(Self::Mfsk22),
            "MFSK31" => Some(Self::Mfsk31),
            "MFSK32" => Some(Self::Mfsk32),
            "MFSK64" => Some(Self::Mfsk64),
            "MFSK64L" => Some(Self::Mfsk64l),
            "MFSK128" => Some(Self::Mfsk128),
            "MFSK128L" => Some(Self::Mfsk128l),
            "Q65" => Some(Self::Q65),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MtoneSubmode {
    ScampOo,
    ScampOoSlw,
}

impl MtoneSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ScampOo => "SCAMP_OO",
            Self::ScampOoSlw => "SCAMP_OO_SLW",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "SCAMP_OO" => Some(Self::ScampOo),
            "SCAMP_OO_SLW" => Some(Self::ScampOoSlw),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OfdmSubmode {
    RibbitPix,
    RibbitSms,
}

impl OfdmSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::RibbitPix => "RIBBIT_PIX",
            Self::RibbitSms => "RIBBIT_SMS",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "RIBBIT_PIX" => Some(Self::RibbitPix),
            "RIBBIT_SMS" => Some(Self::RibbitSms),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OliviaSubmode {
    Olivia4125,
    Olivia4250,
    Olivia8250,
    Olivia8500,
    Olivia16500,
    Olivia161000,
    Olivia321000,
}

impl OliviaSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Olivia4125 => "OLIVIA 4/125",
            Self::Olivia4250 => "OLIVIA 4/250",
            Self::Olivia8250 => "OLIVIA 8/250",
            Self::Olivia8500 => "OLIVIA 8/500",
            Self::Olivia16500 => "OLIVIA 16/500",
            Self::Olivia161000 => "OLIVIA 16/1000",
            Self::Olivia321000 => "OLIVIA 32/1000",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "OLIVIA 4/125" => Some(Self::Olivia4125),
            "OLIVIA 4/250" => Some(Self::Olivia4250),
            "OLIVIA 8/250" => Some(Self::Olivia8250),
            "OLIVIA 8/500" => Some(Self::Olivia8500),
            "OLIVIA 16/500" => Some(Self::Olivia16500),
            "OLIVIA 16/1000" => Some(Self::Olivia161000),
            "OLIVIA 32/1000" => Some(Self::Olivia321000),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperaSubmode {
    OperaBeacon,
    OperaQso,
}

impl OperaSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::OperaBeacon => "OPERA-BEACON",
            Self::OperaQso => "OPERA-QSO",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "OPERA-BEACON" => Some(Self::OperaBeacon),
            "OPERA-QSO" => Some(Self::OperaQso),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PacSubmode {
    Pac2,
    Pac3,
    Pac4,
}

impl PacSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pac2 => "PAC2",
            Self::Pac3 => "PAC3",
            Self::Pac4 => "PAC4",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "PAC2" => Some(Self::Pac2),
            "PAC3" => Some(Self::Pac3),
            "PAC4" => Some(Self::Pac4),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaxSubmode {
    Pax2,
}

impl PaxSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pax2 => "PAX2",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "PAX2" => Some(Self::Pax2),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PskSubmode {
    N8psk125,
    N8psk125f,
    N8psk125fl,
    N8psk250,
    N8psk250f,
    N8psk250fl,
    N8psk500,
    N8psk500f,
    N8psk1000,
    N8psk1000f,
    N8psk1200f,
    Fsk31,
    Psk10,
    Psk31,
    Psk63,
    Psk63f,
    Psk63rc10,
    Psk63rc20,
    Psk63rc32,
    Psk63rc4,
    Psk63rc5,
    Psk125,
    Psk125rc10,
    Psk125rc12,
    Psk125rc16,
    Psk125rc4,
    Psk125rc5,
    Psk250,
    Psk250rc2,
    Psk250rc3,
    Psk250rc5,
    Psk250rc6,
    Psk250rc7,
    Psk500,
    Psk500rc2,
    Psk500rc3,
    Psk500rc4,
    Psk800rc2,
    Psk1000,
    Psk1000rc2,
    Pskam10,
    Pskam31,
    Pskam50,
    Pskfec31,
    Qpsk31,
    Qpsk63,
    Qpsk125,
    Qpsk250,
    Qpsk500,
    Sim31,
    Psk125c12,
    Psk125r,
    Psk250c6,
    Psk250r,
    Psk500c2,
    Psk500c4,
    Psk500r,
    Psk800c2,
    Psk1000c2,
    Psk1000r,
}

impl PskSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::N8psk125 => "8PSK125",
            Self::N8psk125f => "8PSK125F",
            Self::N8psk125fl => "8PSK125FL",
            Self::N8psk250 => "8PSK250",
            Self::N8psk250f => "8PSK250F",
            Self::N8psk250fl => "8PSK250FL",
            Self::N8psk500 => "8PSK500",
            Self::N8psk500f => "8PSK500F",
            Self::N8psk1000 => "8PSK1000",
            Self::N8psk1000f => "8PSK1000F",
            Self::N8psk1200f => "8PSK1200F",
            Self::Fsk31 => "FSK31",
            Self::Psk10 => "PSK10",
            Self::Psk31 => "PSK31",
            Self::Psk63 => "PSK63",
            Self::Psk63f => "PSK63F",
            Self::Psk63rc10 => "PSK63RC10",
            Self::Psk63rc20 => "PSK63RC20",
            Self::Psk63rc32 => "PSK63RC32",
            Self::Psk63rc4 => "PSK63RC4",
            Self::Psk63rc5 => "PSK63RC5",
            Self::Psk125 => "PSK125",
            Self::Psk125rc10 => "PSK125RC10",
            Self::Psk125rc12 => "PSK125RC12",
            Self::Psk125rc16 => "PSK125RC16",
            Self::Psk125rc4 => "PSK125RC4",
            Self::Psk125rc5 => "PSK125RC5",
            Self::Psk250 => "PSK250",
            Self::Psk250rc2 => "PSK250RC2",
            Self::Psk250rc3 => "PSK250RC3",
            Self::Psk250rc5 => "PSK250RC5",
            Self::Psk250rc6 => "PSK250RC6",
            Self::Psk250rc7 => "PSK250RC7",
            Self::Psk500 => "PSK500",
            Self::Psk500rc2 => "PSK500RC2",
            Self::Psk500rc3 => "PSK500RC3",
            Self::Psk500rc4 => "PSK500RC4",
            Self::Psk800rc2 => "PSK800RC2",
            Self::Psk1000 => "PSK1000",
            Self::Psk1000rc2 => "PSK1000RC2",
            Self::Pskam10 => "PSKAM10",
            Self::Pskam31 => "PSKAM31",
            Self::Pskam50 => "PSKAM50",
            Self::Pskfec31 => "PSKFEC31",
            Self::Qpsk31 => "QPSK31",
            Self::Qpsk63 => "QPSK63",
            Self::Qpsk125 => "QPSK125",
            Self::Qpsk250 => "QPSK250",
            Self::Qpsk500 => "QPSK500",
            Self::Sim31 => "SIM31",
            Self::Psk125c12 => "PSK125C12",
            Self::Psk125r => "PSK125R",
            Self::Psk250c6 => "PSK250C6",
            Self::Psk250r => "PSK250R",
            Self::Psk500c2 => "PSK500C2",
            Self::Psk500c4 => "PSK500C4",
            Self::Psk500r => "PSK500R",
            Self::Psk800c2 => "PSK800C2",
            Self::Psk1000c2 => "PSK1000C2",
            Self::Psk1000r => "PSK1000R",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "8PSK125" => Some(Self::N8psk125),
            "8PSK125F" => Some(Self::N8psk125f),
            "8PSK125FL" => Some(Self::N8psk125fl),
            "8PSK250" => Some(Self::N8psk250),
            "8PSK250F" => Some(Self::N8psk250f),
            "8PSK250FL" => Some(Self::N8psk250fl),
            "8PSK500" => Some(Self::N8psk500),
            "8PSK500F" => Some(Self::N8psk500f),
            "8PSK1000" => Some(Self::N8psk1000),
            "8PSK1000F" => Some(Self::N8psk1000f),
            "8PSK1200F" => Some(Self::N8psk1200f),
            "FSK31" => Some(Self::Fsk31),
            "PSK10" => Some(Self::Psk10),
            "PSK31" => Some(Self::Psk31),
            "PSK63" => Some(Self::Psk63),
            "PSK63F" => Some(Self::Psk63f),
            "PSK63RC10" => Some(Self::Psk63rc10),
            "PSK63RC20" => Some(Self::Psk63rc20),
            "PSK63RC32" => Some(Self::Psk63rc32),
            "PSK63RC4" => Some(Self::Psk63rc4),
            "PSK63RC5" => Some(Self::Psk63rc5),
            "PSK125" => Some(Self::Psk125),
            "PSK125RC10" => Some(Self::Psk125rc10),
            "PSK125RC12" => Some(Self::Psk125rc12),
            "PSK125RC16" => Some(Self::Psk125rc16),
            "PSK125RC4" => Some(Self::Psk125rc4),
            "PSK125RC5" => Some(Self::Psk125rc5),
            "PSK250" => Some(Self::Psk250),
            "PSK250RC2" => Some(Self::Psk250rc2),
            "PSK250RC3" => Some(Self::Psk250rc3),
            "PSK250RC5" => Some(Self::Psk250rc5),
            "PSK250RC6" => Some(Self::Psk250rc6),
            "PSK250RC7" => Some(Self::Psk250rc7),
            "PSK500" => Some(Self::Psk500),
            "PSK500RC2" => Some(Self::Psk500rc2),
            "PSK500RC3" => Some(Self::Psk500rc3),
            "PSK500RC4" => Some(Self::Psk500rc4),
            "PSK800RC2" => Some(Self::Psk800rc2),
            "PSK1000" => Some(Self::Psk1000),
            "PSK1000RC2" => Some(Self::Psk1000rc2),
            "PSKAM10" => Some(Self::Pskam10),
            "PSKAM31" => Some(Self::Pskam31),
            "PSKAM50" => Some(Self::Pskam50),
            "PSKFEC31" => Some(Self::Pskfec31),
            "QPSK31" => Some(Self::Qpsk31),
            "QPSK63" => Some(Self::Qpsk63),
            "QPSK125" => Some(Self::Qpsk125),
            "QPSK250" => Some(Self::Qpsk250),
            "QPSK500" => Some(Self::Qpsk500),
            "SIM31" => Some(Self::Sim31),
            "PSK125C12" => Some(Self::Psk125c12),
            "PSK125R" => Some(Self::Psk125r),
            "PSK250C6" => Some(Self::Psk250c6),
            "PSK250R" => Some(Self::Psk250r),
            "PSK500C2" => Some(Self::Psk500c2),
            "PSK500C4" => Some(Self::Psk500c4),
            "PSK500R" => Some(Self::Psk500r),
            "PSK800C2" => Some(Self::Psk800c2),
            "PSK1000C2" => Some(Self::Psk1000c2),
            "PSK1000R" => Some(Self::Psk1000r),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Qra64Submode {
    Qra64a,
    Qra64b,
    Qra64c,
    Qra64d,
    Qra64e,
}

impl Qra64Submode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Qra64a => "QRA64A",
            Self::Qra64b => "QRA64B",
            Self::Qra64c => "QRA64C",
            Self::Qra64d => "QRA64D",
            Self::Qra64e => "QRA64E",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "QRA64A" => Some(Self::Qra64a),
            "QRA64B" => Some(Self::Qra64b),
            "QRA64C" => Some(Self::Qra64c),
            "QRA64D" => Some(Self::Qra64d),
            "QRA64E" => Some(Self::Qra64e),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RosSubmode {
    RosEme,
    RosHf,
    RosMf,
}

impl RosSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::RosEme => "ROS-EME",
            Self::RosHf => "ROS-HF",
            Self::RosMf => "ROS-MF",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "ROS-EME" => Some(Self::RosEme),
            "ROS-HF" => Some(Self::RosHf),
            "ROS-MF" => Some(Self::RosMf),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RttySubmode {
    Asci,
}

impl RttySubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Asci => "ASCI",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "ASCI" => Some(Self::Asci),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SsbSubmode {
    Lsb,
    Usb,
}

impl SsbSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lsb => "LSB",
            Self::Usb => "USB",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "LSB" => Some(Self::Lsb),
            "USB" => Some(Self::Usb),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThorSubmode {
    ThorM,
    Thor4,
    Thor5,
    Thor8,
    Thor11,
    Thor16,
    Thor22,
    Thor25x4,
    Thor50x1,
    Thor50x2,
    Thor100,
}

impl ThorSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ThorM => "THOR-M",
            Self::Thor4 => "THOR4",
            Self::Thor5 => "THOR5",
            Self::Thor8 => "THOR8",
            Self::Thor11 => "THOR11",
            Self::Thor16 => "THOR16",
            Self::Thor22 => "THOR22",
            Self::Thor25x4 => "THOR25X4",
            Self::Thor50x1 => "THOR50X1",
            Self::Thor50x2 => "THOR50X2",
            Self::Thor100 => "THOR100",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "THOR-M" => Some(Self::ThorM),
            "THOR4" => Some(Self::Thor4),
            "THOR5" => Some(Self::Thor5),
            "THOR8" => Some(Self::Thor8),
            "THOR11" => Some(Self::Thor11),
            "THOR16" => Some(Self::Thor16),
            "THOR22" => Some(Self::Thor22),
            "THOR25X4" => Some(Self::Thor25x4),
            "THOR50X1" => Some(Self::Thor50x1),
            "THOR50X2" => Some(Self::Thor50x2),
            "THOR100" => Some(Self::Thor100),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThrbSubmode {
    Thrbx,
    Thrbx1,
    Thrbx2,
    Thrbx4,
    Throb1,
    Throb2,
    Throb4,
}

impl ThrbSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Thrbx => "THRBX",
            Self::Thrbx1 => "THRBX1",
            Self::Thrbx2 => "THRBX2",
            Self::Thrbx4 => "THRBX4",
            Self::Throb1 => "THROB1",
            Self::Throb2 => "THROB2",
            Self::Throb4 => "THROB4",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "THRBX" => Some(Self::Thrbx),
            "THRBX1" => Some(Self::Thrbx1),
            "THRBX2" => Some(Self::Thrbx2),
            "THRBX4" => Some(Self::Thrbx4),
            "THROB1" => Some(Self::Throb1),
            "THROB2" => Some(Self::Throb2),
            "THROB4" => Some(Self::Throb4),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TorSubmode {
    Amtorfec,
    Gtor,
    Navtex,
    Sitorb,
}

impl TorSubmode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Amtorfec => "AMTORFEC",
            Self::Gtor => "GTOR",
            Self::Navtex => "NAVTEX",
            Self::Sitorb => "SITORB",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "AMTORFEC" => Some(Self::Amtorfec),
            "GTOR" => Some(Self::Gtor),
            "NAVTEX" => Some(Self::Navtex),
            "SITORB" => Some(Self::Sitorb),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KnownMode {
    Am,
    Ardop,
    Atv,
    Chip(Option<ChipSubmode>),
    Clo,
    Contesti,
    Cw(Option<CwSubmode>),
    Digitalvoice(Option<DigitalvoiceSubmode>),
    Domino(Option<DominoSubmode>),
    Dynamic(Option<DynamicSubmode>),
    Fax,
    Fm,
    Fsk441,
    Fsk(Option<FskSubmode>),
    Ft8,
    Hell(Option<HellSubmode>),
    Iscat(Option<IscatSubmode>),
    Jt4(Option<Jt4Submode>),
    Jt6m,
    Jt9(Option<Jt9Submode>),
    Jt44,
    Jt65(Option<Jt65Submode>),
    Mfsk(Option<MfskSubmode>),
    Msk144,
    Mtone(Option<MtoneSubmode>),
    Mt63,
    Ofdm(Option<OfdmSubmode>),
    Olivia(Option<OliviaSubmode>),
    Opera(Option<OperaSubmode>),
    Pac(Option<PacSubmode>),
    Pax(Option<PaxSubmode>),
    Pkt,
    Psk(Option<PskSubmode>),
    Psk2k,
    Q15,
    Qra64(Option<Qra64Submode>),
    Ros(Option<RosSubmode>),
    Rtty(Option<RttySubmode>),
    Rttym,
    Ssb(Option<SsbSubmode>),
    Sstv,
    T10,
    Thor(Option<ThorSubmode>),
    Thrb(Option<ThrbSubmode>),
    Tor(Option<TorSubmode>),
    V4,
    Voi,
    Winmor,
    Wspr,
    Amtorfec,
    Asci,
    C4fm,
    Chip64,
    Chip128,
    Dominof,
    Dstar,
    Fmhell,
    Fsk31,
    Gtor,
    Hell80,
    Hfsk,
    Jt4a,
    Jt4b,
    Jt4c,
    Jt4d,
    Jt4e,
    Jt4f,
    Jt4g,
    Jt65a,
    Jt65b,
    Jt65c,
    Mfsk8,
    Mfsk16,
    Pac2,
    Pac3,
    Pax2,
    Pcw,
    Psk10,
    Psk31,
    Psk63,
    Psk63f,
    Psk125,
    Pskam10,
    Pskam31,
    Pskam50,
    Pskfec31,
    Pskhell,
    Qpsk31,
    Qpsk63,
    Qpsk125,
    Thrbx,
}

impl KnownMode {
    pub fn identity(self) -> ModeIdentity {
        match self {
            Self::Am => ModeIdentity::Parent { label: "AM".into() },
            Self::Ardop => ModeIdentity::Parent {
                label: "ARDOP".into(),
            },
            Self::Atv => ModeIdentity::Parent {
                label: "ATV".into(),
            },
            Self::Chip(child) => match child {
                None => ModeIdentity::Parent {
                    label: "CHIP".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("CHIP".into()),
                },
            },
            Self::Clo => ModeIdentity::Parent {
                label: "CLO".into(),
            },
            Self::Contesti => ModeIdentity::Parent {
                label: "CONTESTI".into(),
            },
            Self::Cw(child) => match child {
                None => ModeIdentity::Parent { label: "CW".into() },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("CW".into()),
                },
            },
            Self::Digitalvoice(child) => match child {
                None => ModeIdentity::Parent {
                    label: "DIGITALVOICE".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("DIGITALVOICE".into()),
                },
            },
            Self::Domino(child) => match child {
                None => ModeIdentity::Parent {
                    label: "DOMINO".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("DOMINO".into()),
                },
            },
            Self::Dynamic(child) => match child {
                None => ModeIdentity::Parent {
                    label: "DYNAMIC".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("DYNAMIC".into()),
                },
            },
            Self::Fax => ModeIdentity::Parent {
                label: "FAX".into(),
            },
            Self::Fm => ModeIdentity::Parent { label: "FM".into() },
            Self::Fsk441 => ModeIdentity::Parent {
                label: "FSK441".into(),
            },
            Self::Fsk(child) => match child {
                None => ModeIdentity::Parent {
                    label: "FSK".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("FSK".into()),
                },
            },
            Self::Ft8 => ModeIdentity::Parent {
                label: "FT8".into(),
            },
            Self::Hell(child) => match child {
                None => ModeIdentity::Parent {
                    label: "HELL".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("HELL".into()),
                },
            },
            Self::Iscat(child) => match child {
                None => ModeIdentity::Parent {
                    label: "ISCAT".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("ISCAT".into()),
                },
            },
            Self::Jt4(child) => match child {
                None => ModeIdentity::Parent {
                    label: "JT4".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("JT4".into()),
                },
            },
            Self::Jt6m => ModeIdentity::Parent {
                label: "JT6M".into(),
            },
            Self::Jt9(child) => match child {
                None => ModeIdentity::Parent {
                    label: "JT9".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("JT9".into()),
                },
            },
            Self::Jt44 => ModeIdentity::Parent {
                label: "JT44".into(),
            },
            Self::Jt65(child) => match child {
                None => ModeIdentity::Parent {
                    label: "JT65".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("JT65".into()),
                },
            },
            Self::Mfsk(child) => match child {
                None => ModeIdentity::Parent {
                    label: "MFSK".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("MFSK".into()),
                },
            },
            Self::Msk144 => ModeIdentity::Parent {
                label: "MSK144".into(),
            },
            Self::Mtone(child) => match child {
                None => ModeIdentity::Parent {
                    label: "MTONE".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("MTONE".into()),
                },
            },
            Self::Mt63 => ModeIdentity::Parent {
                label: "MT63".into(),
            },
            Self::Ofdm(child) => match child {
                None => ModeIdentity::Parent {
                    label: "OFDM".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("OFDM".into()),
                },
            },
            Self::Olivia(child) => match child {
                None => ModeIdentity::Parent {
                    label: "OLIVIA".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("OLIVIA".into()),
                },
            },
            Self::Opera(child) => match child {
                None => ModeIdentity::Parent {
                    label: "OPERA".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("OPERA".into()),
                },
            },
            Self::Pac(child) => match child {
                None => ModeIdentity::Parent {
                    label: "PAC".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("PAC".into()),
                },
            },
            Self::Pax(child) => match child {
                None => ModeIdentity::Parent {
                    label: "PAX".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("PAX".into()),
                },
            },
            Self::Pkt => ModeIdentity::Parent {
                label: "PKT".into(),
            },
            Self::Psk(child) => match child {
                None => ModeIdentity::Parent {
                    label: "PSK".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("PSK".into()),
                },
            },
            Self::Psk2k => ModeIdentity::Parent {
                label: "PSK2K".into(),
            },
            Self::Q15 => ModeIdentity::Parent {
                label: "Q15".into(),
            },
            Self::Qra64(child) => match child {
                None => ModeIdentity::Parent {
                    label: "QRA64".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("QRA64".into()),
                },
            },
            Self::Ros(child) => match child {
                None => ModeIdentity::Parent {
                    label: "ROS".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("ROS".into()),
                },
            },
            Self::Rtty(child) => match child {
                None => ModeIdentity::Parent {
                    label: "RTTY".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("RTTY".into()),
                },
            },
            Self::Rttym => ModeIdentity::Parent {
                label: "RTTYM".into(),
            },
            Self::Ssb(child) => match child {
                None => ModeIdentity::Parent {
                    label: "SSB".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("SSB".into()),
                },
            },
            Self::Sstv => ModeIdentity::Parent {
                label: "SSTV".into(),
            },
            Self::T10 => ModeIdentity::Parent {
                label: "T10".into(),
            },
            Self::Thor(child) => match child {
                None => ModeIdentity::Parent {
                    label: "THOR".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("THOR".into()),
                },
            },
            Self::Thrb(child) => match child {
                None => ModeIdentity::Parent {
                    label: "THRB".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("THRB".into()),
                },
            },
            Self::Tor(child) => match child {
                None => ModeIdentity::Parent {
                    label: "TOR".into(),
                },
                Some(child) => ModeIdentity::Specialization {
                    label: child.label().into(),
                    parent: Some("TOR".into()),
                },
            },
            Self::V4 => ModeIdentity::Parent { label: "V4".into() },
            Self::Voi => ModeIdentity::Parent {
                label: "VOI".into(),
            },
            Self::Winmor => ModeIdentity::Parent {
                label: "WINMOR".into(),
            },
            Self::Wspr => ModeIdentity::Parent {
                label: "WSPR".into(),
            },
            Self::Amtorfec => ModeIdentity::Parent {
                label: "AMTORFEC".into(),
            },
            Self::Asci => ModeIdentity::Parent {
                label: "ASCI".into(),
            },
            Self::C4fm => ModeIdentity::Parent {
                label: "C4FM".into(),
            },
            Self::Chip64 => ModeIdentity::Parent {
                label: "CHIP64".into(),
            },
            Self::Chip128 => ModeIdentity::Parent {
                label: "CHIP128".into(),
            },
            Self::Dominof => ModeIdentity::Parent {
                label: "DOMINOF".into(),
            },
            Self::Dstar => ModeIdentity::Parent {
                label: "DSTAR".into(),
            },
            Self::Fmhell => ModeIdentity::Parent {
                label: "FMHELL".into(),
            },
            Self::Fsk31 => ModeIdentity::Parent {
                label: "FSK31".into(),
            },
            Self::Gtor => ModeIdentity::Parent {
                label: "GTOR".into(),
            },
            Self::Hell80 => ModeIdentity::Parent {
                label: "HELL80".into(),
            },
            Self::Hfsk => ModeIdentity::Parent {
                label: "HFSK".into(),
            },
            Self::Jt4a => ModeIdentity::Parent {
                label: "JT4A".into(),
            },
            Self::Jt4b => ModeIdentity::Parent {
                label: "JT4B".into(),
            },
            Self::Jt4c => ModeIdentity::Parent {
                label: "JT4C".into(),
            },
            Self::Jt4d => ModeIdentity::Parent {
                label: "JT4D".into(),
            },
            Self::Jt4e => ModeIdentity::Parent {
                label: "JT4E".into(),
            },
            Self::Jt4f => ModeIdentity::Parent {
                label: "JT4F".into(),
            },
            Self::Jt4g => ModeIdentity::Parent {
                label: "JT4G".into(),
            },
            Self::Jt65a => ModeIdentity::Parent {
                label: "JT65A".into(),
            },
            Self::Jt65b => ModeIdentity::Parent {
                label: "JT65B".into(),
            },
            Self::Jt65c => ModeIdentity::Parent {
                label: "JT65C".into(),
            },
            Self::Mfsk8 => ModeIdentity::Parent {
                label: "MFSK8".into(),
            },
            Self::Mfsk16 => ModeIdentity::Parent {
                label: "MFSK16".into(),
            },
            Self::Pac2 => ModeIdentity::Parent {
                label: "PAC2".into(),
            },
            Self::Pac3 => ModeIdentity::Parent {
                label: "PAC3".into(),
            },
            Self::Pax2 => ModeIdentity::Parent {
                label: "PAX2".into(),
            },
            Self::Pcw => ModeIdentity::Parent {
                label: "PCW".into(),
            },
            Self::Psk10 => ModeIdentity::Parent {
                label: "PSK10".into(),
            },
            Self::Psk31 => ModeIdentity::Parent {
                label: "PSK31".into(),
            },
            Self::Psk63 => ModeIdentity::Parent {
                label: "PSK63".into(),
            },
            Self::Psk63f => ModeIdentity::Parent {
                label: "PSK63F".into(),
            },
            Self::Psk125 => ModeIdentity::Parent {
                label: "PSK125".into(),
            },
            Self::Pskam10 => ModeIdentity::Parent {
                label: "PSKAM10".into(),
            },
            Self::Pskam31 => ModeIdentity::Parent {
                label: "PSKAM31".into(),
            },
            Self::Pskam50 => ModeIdentity::Parent {
                label: "PSKAM50".into(),
            },
            Self::Pskfec31 => ModeIdentity::Parent {
                label: "PSKFEC31".into(),
            },
            Self::Pskhell => ModeIdentity::Parent {
                label: "PSKHELL".into(),
            },
            Self::Qpsk31 => ModeIdentity::Parent {
                label: "QPSK31".into(),
            },
            Self::Qpsk63 => ModeIdentity::Parent {
                label: "QPSK63".into(),
            },
            Self::Qpsk125 => ModeIdentity::Parent {
                label: "QPSK125".into(),
            },
            Self::Thrbx => ModeIdentity::Parent {
                label: "THRBX".into(),
            },
        }
    }

    pub fn from_parent(label: &str) -> Option<Self> {
        match label {
            "AM" => Some(Self::Am),
            "ARDOP" => Some(Self::Ardop),
            "ATV" => Some(Self::Atv),
            "CHIP" => Some(Self::Chip(None)),
            "CLO" => Some(Self::Clo),
            "CONTESTI" => Some(Self::Contesti),
            "CW" => Some(Self::Cw(None)),
            "DIGITALVOICE" => Some(Self::Digitalvoice(None)),
            "DOMINO" => Some(Self::Domino(None)),
            "DYNAMIC" => Some(Self::Dynamic(None)),
            "FAX" => Some(Self::Fax),
            "FM" => Some(Self::Fm),
            "FSK441" => Some(Self::Fsk441),
            "FSK" => Some(Self::Fsk(None)),
            "FT8" => Some(Self::Ft8),
            "HELL" => Some(Self::Hell(None)),
            "ISCAT" => Some(Self::Iscat(None)),
            "JT4" => Some(Self::Jt4(None)),
            "JT6M" => Some(Self::Jt6m),
            "JT9" => Some(Self::Jt9(None)),
            "JT44" => Some(Self::Jt44),
            "JT65" => Some(Self::Jt65(None)),
            "MFSK" => Some(Self::Mfsk(None)),
            "MSK144" => Some(Self::Msk144),
            "MTONE" => Some(Self::Mtone(None)),
            "MT63" => Some(Self::Mt63),
            "OFDM" => Some(Self::Ofdm(None)),
            "OLIVIA" => Some(Self::Olivia(None)),
            "OPERA" => Some(Self::Opera(None)),
            "PAC" => Some(Self::Pac(None)),
            "PAX" => Some(Self::Pax(None)),
            "PKT" => Some(Self::Pkt),
            "PSK" => Some(Self::Psk(None)),
            "PSK2K" => Some(Self::Psk2k),
            "Q15" => Some(Self::Q15),
            "QRA64" => Some(Self::Qra64(None)),
            "ROS" => Some(Self::Ros(None)),
            "RTTY" => Some(Self::Rtty(None)),
            "RTTYM" => Some(Self::Rttym),
            "SSB" => Some(Self::Ssb(None)),
            "SSTV" => Some(Self::Sstv),
            "T10" => Some(Self::T10),
            "THOR" => Some(Self::Thor(None)),
            "THRB" => Some(Self::Thrb(None)),
            "TOR" => Some(Self::Tor(None)),
            "V4" => Some(Self::V4),
            "VOI" => Some(Self::Voi),
            "WINMOR" => Some(Self::Winmor),
            "WSPR" => Some(Self::Wspr),
            "AMTORFEC" => Some(Self::Amtorfec),
            "ASCI" => Some(Self::Asci),
            "C4FM" => Some(Self::C4fm),
            "CHIP64" => Some(Self::Chip64),
            "CHIP128" => Some(Self::Chip128),
            "DOMINOF" => Some(Self::Dominof),
            "DSTAR" => Some(Self::Dstar),
            "FMHELL" => Some(Self::Fmhell),
            "FSK31" => Some(Self::Fsk31),
            "GTOR" => Some(Self::Gtor),
            "HELL80" => Some(Self::Hell80),
            "HFSK" => Some(Self::Hfsk),
            "JT4A" => Some(Self::Jt4a),
            "JT4B" => Some(Self::Jt4b),
            "JT4C" => Some(Self::Jt4c),
            "JT4D" => Some(Self::Jt4d),
            "JT4E" => Some(Self::Jt4e),
            "JT4F" => Some(Self::Jt4f),
            "JT4G" => Some(Self::Jt4g),
            "JT65A" => Some(Self::Jt65a),
            "JT65B" => Some(Self::Jt65b),
            "JT65C" => Some(Self::Jt65c),
            "MFSK8" => Some(Self::Mfsk8),
            "MFSK16" => Some(Self::Mfsk16),
            "PAC2" => Some(Self::Pac2),
            "PAC3" => Some(Self::Pac3),
            "PAX2" => Some(Self::Pax2),
            "PCW" => Some(Self::Pcw),
            "PSK10" => Some(Self::Psk10),
            "PSK31" => Some(Self::Psk31),
            "PSK63" => Some(Self::Psk63),
            "PSK63F" => Some(Self::Psk63f),
            "PSK125" => Some(Self::Psk125),
            "PSKAM10" => Some(Self::Pskam10),
            "PSKAM31" => Some(Self::Pskam31),
            "PSKAM50" => Some(Self::Pskam50),
            "PSKFEC31" => Some(Self::Pskfec31),
            "PSKHELL" => Some(Self::Pskhell),
            "QPSK31" => Some(Self::Qpsk31),
            "QPSK63" => Some(Self::Qpsk63),
            "QPSK125" => Some(Self::Qpsk125),
            "THRBX" => Some(Self::Thrbx),
            _ => None,
        }
    }

    pub fn from_specialization(label: &str, parent: &str) -> Option<Self> {
        match parent {
            "CHIP" => ChipSubmode::from_label(label).map(|child| Self::Chip(Some(child))),
            "CW" => CwSubmode::from_label(label).map(|child| Self::Cw(Some(child))),
            "DIGITALVOICE" => {
                DigitalvoiceSubmode::from_label(label).map(|child| Self::Digitalvoice(Some(child)))
            }
            "DOMINO" => DominoSubmode::from_label(label).map(|child| Self::Domino(Some(child))),
            "DYNAMIC" => DynamicSubmode::from_label(label).map(|child| Self::Dynamic(Some(child))),
            "FSK" => FskSubmode::from_label(label).map(|child| Self::Fsk(Some(child))),
            "HELL" => HellSubmode::from_label(label).map(|child| Self::Hell(Some(child))),
            "ISCAT" => IscatSubmode::from_label(label).map(|child| Self::Iscat(Some(child))),
            "JT4" => Jt4Submode::from_label(label).map(|child| Self::Jt4(Some(child))),
            "JT9" => Jt9Submode::from_label(label).map(|child| Self::Jt9(Some(child))),
            "JT65" => Jt65Submode::from_label(label).map(|child| Self::Jt65(Some(child))),
            "MFSK" => MfskSubmode::from_label(label).map(|child| Self::Mfsk(Some(child))),
            "MTONE" => MtoneSubmode::from_label(label).map(|child| Self::Mtone(Some(child))),
            "OFDM" => OfdmSubmode::from_label(label).map(|child| Self::Ofdm(Some(child))),
            "OLIVIA" => OliviaSubmode::from_label(label).map(|child| Self::Olivia(Some(child))),
            "OPERA" => OperaSubmode::from_label(label).map(|child| Self::Opera(Some(child))),
            "PAC" => PacSubmode::from_label(label).map(|child| Self::Pac(Some(child))),
            "PAX" => PaxSubmode::from_label(label).map(|child| Self::Pax(Some(child))),
            "PSK" => PskSubmode::from_label(label).map(|child| Self::Psk(Some(child))),
            "QRA64" => Qra64Submode::from_label(label).map(|child| Self::Qra64(Some(child))),
            "ROS" => RosSubmode::from_label(label).map(|child| Self::Ros(Some(child))),
            "RTTY" => RttySubmode::from_label(label).map(|child| Self::Rtty(Some(child))),
            "SSB" => SsbSubmode::from_label(label).map(|child| Self::Ssb(Some(child))),
            "THOR" => ThorSubmode::from_label(label).map(|child| Self::Thor(Some(child))),
            "THRB" => ThrbSubmode::from_label(label).map(|child| Self::Thrb(Some(child))),
            "TOR" => TorSubmode::from_label(label).map(|child| Self::Tor(Some(child))),
            _ => None,
        }
    }
}
