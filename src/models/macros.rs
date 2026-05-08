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

/// Implements `Visitable` for a type with no schema children (a leaf node).
macro_rules! visitable_leaf {
    ($name:ident) => {
        impl Visitable for $name {
            fn for_each_child(&self, _f: &mut dyn FnMut(&dyn Visitable)) {}
            fn for_each_child_mut(&mut self, _f: &mut dyn FnMut(&mut dyn Visitable)) {}
        }
    };
}

/// Implements `Visitable` for a type whose children are the items of `self.$field` (a Vec or
/// similar iterable).
macro_rules! visitable_forwarded_iter {
    ($name:ident, $field:ident) => {
        impl Visitable for $name {
            fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
                for item in &self.$field {
                    f(item);
                }
            }
            fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
                for item in &mut self.$field {
                    f(item);
                }
            }
        }
    };
}

/// Implements `Visitable` for an enum whose sole job is to wrap one of several concrete types.
/// Each variant must be a newtype `EnumName::VariantName(inner)`.
/// The single child is the inner value — traversal continues into it.
macro_rules! visitable_dispatch_enum {
    ($name:ident, $($variant:ident),+ $(,)?) => {
        impl Visitable for $name {
            fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
                match self {
                    $($name::$variant(v) => f(v),)+
                }
            }
            fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
                match self {
                    $($name::$variant(v) => f(v),)+
                }
            }
        }
    };
}

pub(crate) use literal_enum;
pub(crate) use visitable_dispatch_enum;
pub(crate) use visitable_forwarded_iter;
pub(crate) use visitable_leaf;
