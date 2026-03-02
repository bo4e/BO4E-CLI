# Visitor Pattern Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the two unfinished iterator-based traversal files with a single closure-based Visitor pattern in `visitable.rs` that compiles without `RefCell` and mirrors the `Iterator::try_for_each` API.

**Architecture:** The `Visitable` trait exposes two object-safe closure methods (`for_each_child` / `for_each_child_mut`). Four generic traversal methods live in `impl dyn Visitable` (not on the trait itself) so they can be generic over `T: Any` and `R: Try<Output = ()>` without breaking object safety. All schema types implement the trait via updated macros or short manual impls.

**Tech Stack:** Rust 1.89, `std::ops::{ControlFlow, Try}`, `std::any::Any`, `cargo test`

---

### Task 1: Rewrite `src/utils/visitable.rs`

**Files:**
- Modify: `src/utils/visitable.rs` (full replacement)

This task writes the new trait, all four traversal methods, and the complete test suite in one go. The file will compile on its own; dependent files will fail until Tasks 3–5.

**Step 1: Replace the entire file**

```rust
use std::any::Any;
use std::fmt::Debug;
use std::ops::{ControlFlow, Try};

/// A trait for types that form a tree, traversable with closures.
///
/// By accepting closures rather than returning iterators this trait is object-safe
/// and avoids runtime borrow checking (RefCell) for mutable traversal.
pub trait Visitable: Any + Debug {
    /// Visit each direct child of this node.
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable));
    /// Visit each direct child of this node with mutable access.
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable));
}

impl dyn Visitable {
    /// Visit every node in the subtree whose concrete type is `T`.
    /// Traversal is depth-first pre-order (self before children).
    pub fn visit_all<T: Any>(&self, f: &mut dyn FnMut(&T)) {
        if let Some(t) = (self as &dyn Any).downcast_ref::<T>() {
            f(t);
        }
        self.for_each_child(&mut |child| child.visit_all(f));
    }

    /// Like `visit_all`, but the closure can stop traversal early.
    /// Mirrors `Iterator::try_for_each`: the closure and the method both return `R: Try<Output = ()>`.
    pub fn try_visit_all<T: Any, R: Try<Output = ()>>(
        &self,
        f: &mut dyn FnMut(&T) -> R,
    ) -> R {
        if let Some(t) = (self as &dyn Any).downcast_ref::<T>() {
            match f(t).branch() {
                ControlFlow::Continue(()) => {}
                ControlFlow::Break(residual) => return R::from_residual(residual),
            }
        }
        let mut residual: Option<R::Residual> = None;
        self.for_each_child(&mut |child| {
            if residual.is_none() {
                if let ControlFlow::Break(r) = child.try_visit_all::<T, R>(f).branch() {
                    residual = Some(r);
                }
            }
        });
        match residual {
            Some(r) => R::from_residual(r),
            None => R::from_output(()),
        }
    }

    /// Mutably visit every node in the subtree whose concrete type is `T`.
    pub fn visit_all_mut<T: Any>(&mut self, f: &mut dyn FnMut(&mut T)) {
        {
            // Inner block: drop self_any before for_each_child_mut re-borrows self.
            let self_any: &mut dyn Any = self;
            if let Some(t) = self_any.downcast_mut::<T>() {
                f(t);
            }
        }
        self.for_each_child_mut(&mut |child| child.visit_all_mut(f));
    }

    /// Like `visit_all_mut`, but the closure can stop traversal early.
    /// Mirrors `Iterator::try_for_each`.
    pub fn try_visit_all_mut<T: Any, R: Try<Output = ()>>(
        &mut self,
        f: &mut dyn FnMut(&mut T) -> R,
    ) -> R {
        {
            let self_any: &mut dyn Any = self;
            if let Some(t) = self_any.downcast_mut::<T>() {
                match f(t).branch() {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(residual) => return R::from_residual(residual),
                }
            }
        }
        let mut residual: Option<R::Residual> = None;
        self.for_each_child_mut(&mut |child| {
            if residual.is_none() {
                if let ControlFlow::Break(r) = child.try_visit_all_mut::<T, R>(f).branch() {
                    residual = Some(r);
                }
            }
        });
        match residual {
            Some(r) => R::from_residual(r),
            None => R::from_output(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::ControlFlow;

    // ── minimal test tree ──────────────────────────────────────────────────────

    #[derive(Debug, Clone, PartialEq)]
    struct Leaf(i32);

    impl Visitable for Leaf {
        fn for_each_child(&self, _f: &mut dyn FnMut(&dyn Visitable)) {}
        fn for_each_child_mut(&mut self, _f: &mut dyn FnMut(&mut dyn Visitable)) {}
    }

    #[derive(Debug)]
    struct Branch {
        tag: &'static str,
        children: Vec<Leaf>,
    }

    impl Visitable for Branch {
        fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
            for child in &self.children {
                f(child);
            }
        }
        fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
            for child in &mut self.children {
                f(child);
            }
        }
    }

    #[derive(Debug)]
    struct Tree {
        left: Branch,
        right: Branch,
    }

    impl Visitable for Tree {
        fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
            f(&self.left);
            f(&self.right);
        }
        fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
            f(&mut self.left);
            f(&mut self.right);
        }
    }

    /// Tree { left: [Leaf(1), Leaf(2)], right: [Leaf(3)] }
    fn make_tree() -> Tree {
        Tree {
            left: Branch { tag: "left", children: vec![Leaf(1), Leaf(2)] },
            right: Branch { tag: "right", children: vec![Leaf(3)] },
        }
    }

    // ── visit_all ─────────────────────────────────────────────────────────────

    #[test]
    fn visit_all_visits_self_when_type_matches() {
        let leaf = Leaf(42);
        let mut seen = vec![];
        ((&leaf) as &dyn Visitable).visit_all::<Leaf>(&mut |l| seen.push(l.0));
        assert_eq!(seen, [42]);
    }

    #[test]
    fn visit_all_collects_all_matching_nodes() {
        let tree = make_tree();
        let mut values = vec![];
        ((&tree) as &dyn Visitable).visit_all::<Leaf>(&mut |l| values.push(l.0));
        values.sort_unstable();
        assert_eq!(values, [1, 2, 3]);
    }

    #[test]
    fn visit_all_only_visits_matching_type() {
        let tree = make_tree();
        let mut branch_count = 0usize;
        ((&tree) as &dyn Visitable).visit_all::<Branch>(&mut |_| branch_count += 1);
        assert_eq!(branch_count, 2);

        let mut leaf_count = 0usize;
        ((&tree) as &dyn Visitable).visit_all::<Leaf>(&mut |_| leaf_count += 1);
        assert_eq!(leaf_count, 3);
    }

    #[test]
    fn visit_all_is_depth_first_preorder() {
        let tree = make_tree();
        let mut order: Vec<&str> = vec![];
        ((&tree) as &dyn Visitable).visit_all::<Branch>(&mut |b| order.push(b.tag));
        assert_eq!(order, ["left", "right"]);
    }

    // ── visit_all_mut ─────────────────────────────────────────────────────────

    #[test]
    fn visit_all_mut_modifies_all_matching_nodes() {
        let mut tree = make_tree();
        ((&mut tree) as &mut dyn Visitable).visit_all_mut::<Leaf>(&mut |l| l.0 *= 10);
        let mut values = vec![];
        ((&tree) as &dyn Visitable).visit_all::<Leaf>(&mut |l| values.push(l.0));
        values.sort_unstable();
        assert_eq!(values, [10, 20, 30]);
    }

    // ── try_visit_all_mut ─────────────────────────────────────────────────────

    #[test]
    fn try_visit_all_mut_with_control_flow_stops_on_break() {
        let mut tree = make_tree();
        let mut visited = vec![];
        let result = ((&mut tree) as &mut dyn Visitable)
            .try_visit_all_mut::<Leaf, ControlFlow<i32>>(&mut |l| {
                visited.push(l.0);
                if l.0 == 2 {
                    ControlFlow::Break(l.0)
                } else {
                    ControlFlow::Continue(())
                }
            });
        assert_eq!(result, ControlFlow::Break(2));
        assert_eq!(visited, [1, 2]); // Leaf(3) was never reached
    }

    #[test]
    fn try_visit_all_mut_with_result_stops_on_err() {
        let mut tree = make_tree();
        let mut visited = vec![];
        let result: Result<(), String> = ((&mut tree) as &mut dyn Visitable)
            .try_visit_all_mut::<Leaf, _>(&mut |l| {
                visited.push(l.0);
                if l.0 == 2 {
                    Err("stop".to_string())
                } else {
                    Ok(())
                }
            });
        assert_eq!(result, Err("stop".to_string()));
        assert_eq!(visited, [1, 2]); // Leaf(3) was never reached
    }

    #[test]
    fn try_visit_all_mut_returns_ok_when_no_error() {
        let mut tree = make_tree();
        let result: Result<(), String> =
            ((&mut tree) as &mut dyn Visitable).try_visit_all_mut::<Leaf, _>(&mut |_| Ok(()));
        assert_eq!(result, Ok(()));
    }
}
```

**Step 2: Verify the file itself compiles in isolation**

```bash
cargo check 2>&1 | grep "visitable"
```

Expected: errors only from *other* files (json_schema.rs, update_refs.rs) that still use the old API — not from visitable.rs itself.

---

### Task 2: Delete old traverse files and update `src/utils.rs`

**Files:**
- Delete: `src/utils/traverse.rs`
- Delete: `src/utils/traverse2.rs`
- Modify: `src/utils.rs`

**Step 1: Delete the two files**

```bash
rm src/utils/traverse.rs src/utils/traverse2.rs
```

**Step 2: Replace `src/utils.rs`**

```rust
pub mod tokio;
pub mod visitable;
```

---

### Task 3: Update `src/models/macros.rs`

**Files:**
- Modify: `src/models/macros.rs`

Replace the entire file. The three existing macros are updated to generate `for_each_child`/`for_each_child_mut`. A fourth macro, `visitable_dispatch_enum!`, is added for enum wrapper types.

**Step 1: Replace the entire file**

```rust
use std::iter::empty;

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

/// Implements `Visitable` for a type that has exactly one schema child at `self.$field`.
macro_rules! visitable_forwarded {
    ($name:ident, $field:ident) => {
        impl Visitable for $name {
            fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
                f(&self.$field);
            }
            fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
                f(&mut self.$field);
            }
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
pub(crate) use visitable_forwarded;
pub(crate) use visitable_forwarded_iter;
pub(crate) use visitable_leaf;
```

---

### Task 4: Update `src/models/json_schema.rs`

**Files:**
- Modify: `src/models/json_schema.rs`

Three changes in this file:
1. Update the macro import to include `visitable_dispatch_enum`.
2. Replace all `impl Visitable` blocks with the new API.
3. Update the two visitor-related tests.

**Step 1: Update the macro import at the top of the file**

Find:
```rust
use crate::models::macros::{
    literal_enum, visitable_forwarded, visitable_forwarded_iter, visitable_leaf,
};
```

Replace with:
```rust
use crate::models::macros::{
    literal_enum, visitable_dispatch_enum, visitable_forwarded, visitable_forwarded_iter,
    visitable_leaf,
};
```

**Step 2: Remove all old `impl Visitable` blocks**

Delete every `impl Visitable for …` block and every `visitable_leaf!(…)`, `visitable_forwarded!(…)`, `visitable_forwarded_iter!(…)` call in the file.

**Step 3: Add new `impl Visitable` blocks after all struct/enum definitions**

Paste after the last struct/enum definition and before `#[cfg(test)]`:

```rust
// ── Leaf types (no schema children) ──────────────────────────────────────────
visitable_leaf!(TypeBase);
visitable_leaf!(SchemaRootTypeBase);
visitable_leaf!(StrEnumSchema);
visitable_leaf!(StringSchema);
visitable_leaf!(ConstantSchema);
visitable_leaf!(NumberSchema);
visitable_leaf!(DecimalSchema);
visitable_leaf!(IntegerSchema);
visitable_leaf!(BooleanSchema);
visitable_leaf!(NullSchema);
visitable_leaf!(AnySchema);
visitable_leaf!(ReferenceSchema);

// ── Collection types ──────────────────────────────────────────────────────────
visitable_forwarded_iter!(AnyOfSchema, any_of);
visitable_forwarded_iter!(AllOfSchema, all_of);

// ── ObjectSchema: children are its property values ────────────────────────────
impl Visitable for ObjectSchema {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        for value in self.properties.values() {
            f(value);
        }
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        for value in self.properties.values_mut() {
            f(value);
        }
    }
}

// ── ArraySchema: single child is the boxed item type ─────────────────────────
impl Visitable for ArraySchema {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        f(&*self.items);
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        f(&mut *self.items);
    }
}

// ── Root types: inner schema + any inline $defs ───────────────────────────────
impl Visitable for SchemaRootObject {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        f(&self.object);
        for value in self.base.defs.values() {
            f(value);
        }
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        f(&mut self.object);
        for value in self.base.defs.values_mut() {
            f(value);
        }
    }
}

impl Visitable for SchemaRootStrEnum {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        f(&self.str_enum);
        for value in self.base.defs.values() {
            f(value);
        }
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        f(&mut self.str_enum);
        for value in self.base.defs.values_mut() {
            f(value);
        }
    }
}

// ── Enum wrappers: dispatch to the single inner value ────────────────────────
visitable_dispatch_enum!(
    SchemaType,
    Object,
    StrEnum,
    Array,
    AnyOf,
    AllOf,
    StringSchema,
    ConstantSchema,
    NumberSchema,
    DecimalSchema,
    IntegerSchema,
    BooleanSchema,
    NullSchema,
    ReferenceSchema,
    AnySchema,
);
visitable_dispatch_enum!(SchemaClassType, Object, StrEnum);
visitable_dispatch_enum!(SchemaRootType, Object, StrEnum);
```

**Step 4: Update the visitor tests inside `#[cfg(test)]`**

Replace the `get_ref_strings` helper and the two visitor tests. Remove the `use std::cell::RefCell;` import if it is no longer used elsewhere in the test module.

```rust
fn get_ref_strings(schema: &SchemaRootObject) -> HashSet<String> {
    let mut refs = HashSet::new();
    (schema as &dyn Visitable).visit_all::<ReferenceSchema>(&mut |r| {
        refs.insert(r.r#ref.clone());
    });
    refs
}

#[test]
fn test_complex_root_object_visit_trait() {
    let schema = get_example_schema();
    let refs = get_ref_strings(&schema);
    println!("{}", serde_json::to_string_pretty(&refs).unwrap());
    assert_eq!(refs.len(), 2);
}

#[test]
fn test_complex_root_object_visit_and_mutate() {
    let mut schema = get_example_schema();

    let ref_online_regex = regex::Regex::new(
        "^https://raw\\.githubusercontent\\.com/BO4E/BO4E-Schemas/\
        (?P<version>[^/]+)/\
        src/bo4e_schemas/(?P<sub_path>(?:\\w+/)*)(?P<model>\\w+)\\.json#?$",
    )
    .unwrap();

    ((&mut schema) as &mut dyn Visitable).visit_all_mut::<ReferenceSchema>(&mut |r| {
        r.r#ref = ref_online_regex
            .replace(&r.r#ref, "../${sub_path}${model}.json")
            .to_string();
    });

    let refs = get_ref_strings(&schema);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    assert_eq!(
        refs,
        HashSet::from(["../bo/Geschaeftspartner.json".to_string()])
    );
}
```

**Step 5: Verify compilation**

```bash
cargo check 2>&1 | grep -v "update_refs"
```

Expected: no errors except possibly in `update_refs.rs` (handled next task).

---

### Task 5: Update `src/edit/update_refs.rs`

**Files:**
- Modify: `src/edit/update_refs.rs`

**Step 1: Replace the entire file**

```rust
use crate::models::json_schema::ReferenceSchema;
use crate::models::schema_meta::{Schema, Schemas};
use crate::utils::visitable::Visitable;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ops::DerefMut;

lazy_static! {
    pub static ref REF_ONLINE_REGEX: regex::Regex = regex::Regex::new(
        r"^https://raw\.githubusercontent\.com/(?:BO4E|bo4e|Bo4e|Hochfrequenz)/BO4E-Schemas/(?P<version>[^/]+)/src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
    )
    .unwrap();
    pub static ref REF_DEFS_REGEX: regex::Regex =
        regex::Regex::new(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$").unwrap();
}

fn update_reference(
    _reference: &mut ReferenceSchema,
    _current_module: &[String],
    _namespace: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    Ok(()) // TODO: implement reference rewriting logic
}

fn update_references_single(
    schema: &mut Schema,
    namespace: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let module: Vec<String> = schema.module().iter().cloned().collect();
    let visitable: &mut dyn Visitable = schema.schema_mut()?;
    visitable.try_visit_all_mut::<ReferenceSchema, Result<(), String>>(
        &mut |reference| update_reference(reference, &module, namespace),
    )
}

pub fn update_references_all(schemas: &mut Schemas) -> Result<(), String> {
    let namespace = schemas.modules_by_name();
    for schema in schemas.iter_mut() {
        update_references_single(schema.borrow_mut().deref_mut(), &namespace)?;
    }
    Ok(())
}
```

---

### Task 6: Verify everything compiles and all tests pass, then commit

**Step 1: Full compilation check**

```bash
cargo check
```

Expected: no errors.

**Step 2: Run all tests**

```bash
cargo test --lib
```

Expected output (subset):
```
test utils::visitable::tests::visit_all_visits_self_when_type_matches ... ok
test utils::visitable::tests::visit_all_collects_all_matching_nodes ... ok
test utils::visitable::tests::visit_all_only_visits_matching_type ... ok
test utils::visitable::tests::visit_all_is_depth_first_preorder ... ok
test utils::visitable::tests::visit_all_mut_modifies_all_matching_nodes ... ok
test utils::visitable::tests::try_visit_all_mut_with_control_flow_stops_on_break ... ok
test utils::visitable::tests::try_visit_all_mut_with_result_stops_on_err ... ok
test utils::visitable::tests::try_visit_all_mut_returns_ok_when_no_error ... ok
test models::json_schema::tests::test_complex_root_object_schema_serialization_roundtrip ... ok
test models::json_schema::tests::test_complex_root_object_visit_trait ... ok
test models::json_schema::tests::test_complex_root_object_visit_and_mutate ... ok
```

**Step 3: Commit**

```bash
git add src/utils/visitable.rs src/utils.rs src/models/macros.rs src/models/json_schema.rs src/edit/update_refs.rs docs/plans/2026-03-02-visitor-pattern-design.md docs/plans/2026-03-02-visitor-pattern.md
git rm src/utils/traverse.rs src/utils/traverse2.rs
git commit -m "refactor(utils): replace iterator traversal with closure-based Visitor pattern

- Rewrites visitable.rs: object-safe Visitable trait + impl dyn Visitable
  with visit_all, try_visit_all, visit_all_mut, try_visit_all_mut
- try_visit_* mirrors Iterator::try_for_each (R: Try<Output = ()>)
- Deletes traverse.rs and traverse2.rs (unfinished iterator attempts)
- Adds visitable_dispatch_enum! macro for SchemaType/ClassType/RootType
- Updates all json_schema Visitable impls and tests
- Adapts update_refs.rs call site to new API"
```
