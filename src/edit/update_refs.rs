use lazy_static::lazy_static;

// REF_ONLINE_REGEX = re.compile(
//     rf"^https://raw\.githubusercontent\.com/(?:{OWNER.upper()}|{OWNER.lower()}|{OWNER.capitalize()}|Hochfrequenz)/"
//     rf"{REPO}/(?P<version>[^/]+)/"
//     r"src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
// )
// # e.g. https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
// REF_DEFS_REGEX = re.compile(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$")
lazy_static! {
    pub static ref REF_ONLINE_REGEX: regex::Regex = regex::Regex::new(
        r"^https://raw\.githubusercontent\.com/(?:BO4E|bo4e|Bo4e|Hochfrequenz)/BO4E-Schemas/(?P<version>[^/]+)/src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
    )
    .unwrap();
    // e.g. https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
    pub static ref REF_DEFS_REGEX: regex::Regex =
        regex::Regex::new(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$").unwrap();
}
