use std::iter::empty;
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

macro_rules! visitable_leaf {
    ($name:ident) => {
        impl Visitable for $name {
            fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
                Box::new(empty::<&dyn Visitable>())
            }
            fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
                Box::new(empty::<&mut dyn Visitable>())
            }
        }
    };
}

macro_rules! visitable_forwarded {
    ($name:ident, $forwarded_field:ident) => {
        impl Visitable for $name {
            fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
                self.$forwarded_field.sub_nodes()
            }
            fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
                self.$forwarded_field.sub_nodes_mut()
            }
        }
    };
}

macro_rules! visitable_forwarded_iter {
    ($name:ident, $forwarded_field:ident) => {
        impl Visitable for $name {
            fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
                Box::new(
                    self.$forwarded_field
                        .iter()
                        .map(|schema| schema as &dyn Visitable),
                )
            }
            fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
                Box::new(
                    self.$forwarded_field
                        .iter_mut()
                        .map(|schema| schema as &mut dyn Visitable),
                )
            }
        }
    };
}

pub(crate) use literal_enum;
pub(crate) use visitable_forwarded;
pub(crate) use visitable_forwarded_iter;
pub(crate) use visitable_leaf;
