//#[macro_export]
macro_rules! literal_enum {
    ($name:ident, $variant:ident) => {
        #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
        #[serde(rename_all = "camelCase")]
        pub enum $name {
            $variant,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$variant
            }
        }
    };
}

pub(crate) use literal_enum;
