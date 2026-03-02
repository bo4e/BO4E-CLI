# Visitor Pattern for Schema Traversal

**Date:** 2026-03-02
**Branch:** `rust`
**Scope:** `src/utils/visitable.rs`, `src/models/macros.rs`, `src/models/json_schema.rs`, `src/edit/update_refs.rs`

---

## Problem

The Rust branch has two unfinished attempts at tree traversal (`traverse.rs`, `traverse2.rs`) and a partially working `visitable.rs`. All three use iterator-based mutable traversal, which requires runtime borrow checking (`RefCell`) to be sound — something the author explicitly wants to avoid.

The specific failure: `visit_by_type_mut` tried to return a chained iterator whose items borrow from temporary sub-nodes. This creates lifetime errors the borrow checker correctly rejects.

---

## Solution

Pass a **closure into the tree** instead of pulling values **out** as an iterator. Processing one child at a time via a closure is sequentially safe at compile time — no `RefCell` needed.

---

## Trait Design

```rust
pub trait Visitable: Any + Debug {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable));
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable));
}
```

- **Object-safe** — no generic parameters on trait methods; closures are erased to `dyn FnMut`.
- `Any` supertrait enables runtime downcasting to a concrete type inside the traversal methods.
- `Debug` supertrait is already required by the existing code.

---

## Traversal Methods

Defined in `impl dyn Visitable` (not on the trait), so they can be generic over `T` and `R` without breaking object safety. Called on any `&dyn Visitable` or `&mut dyn Visitable` value.

```rust
impl dyn Visitable {
    // Immutable, visit all matching nodes
    pub fn visit_all<T: Any>(&self, f: &mut dyn FnMut(&T));

    // Immutable, stop early — mirrors Iterator::try_for_each
    pub fn try_visit_all<T: Any, R: Try<Output = ()>>(
        &self,
        f: &mut dyn FnMut(&T) -> R,
    ) -> R;

    // Mutable, visit all matching nodes
    pub fn visit_all_mut<T: Any>(&mut self, f: &mut dyn FnMut(&mut T));

    // Mutable, stop early — mirrors Iterator::try_for_each
    pub fn try_visit_all_mut<T: Any, R: Try<Output = ()>>(
        &mut self,
        f: &mut dyn FnMut(&mut T) -> R,
    ) -> R;
}
```

All four traverse the tree in **depth-first pre-order** (self before children).

**`R: Try<Output = ()>` — same bound as `Iterator::try_for_each`**
The return type of the closure and the return type of the method are both `R`. Callers can use any type that implements `Try<Output = ()>`: `ControlFlow<B>`, `Result<(), E>`, or `Option<()>`. The method uses `R::branch()` internally to decide whether to continue, `R::from_residual()` to propagate an early exit, and `R::from_output(())` to construct the success return value. No `?` desugaring is needed — `branch()` is called explicitly.

**Why `dyn FnMut` arguments instead of a second generic on the method?**
The recursive calls inside each method need to pass the same closure down to children. With `dyn FnMut` the concrete closure type is erased at the call site, so the recursive call compiles without additional monomorphisation at each level. `R` remains concrete (monomorphised per call site); only the closure implementor is erased.

---

## Borrow Safety in Mutable Methods

`visit_all_mut` uses a scoped inner block to satisfy the borrow checker without `unsafe`:

```rust
pub fn visit_all_mut<T: Any>(&mut self, f: &mut dyn FnMut(&mut T)) {
    {
        let self_any: &mut dyn Any = self;   // trait upcasting (stable Rust 1.76+)
        if let Some(t) = self_any.downcast_mut::<T>() {
            f(t);
        }
        // self_any and t are dropped here — self is free
    }
    self.for_each_child_mut(&mut |child| child.visit_all_mut(f));
}
```

The inner block ensures `self_any` (and thus `t`) is dropped before `self.for_each_child_mut(...)` borrows `self` again. NLL handles this correctly in the Rust 2024 edition.

---

## Files Changed

### `src/utils/visitable.rs` — complete rewrite

Contains the trait, `impl dyn Visitable`, and the test module.

### `src/utils/traverse.rs`, `src/utils/traverse2.rs` — deleted

Both files contained unfinished iterator-based attempts. All traversal logic moves into `visitable.rs`.

### `src/utils.rs` — remove two module declarations

```rust
// remove:
//mod traverse;
mod traverse2;
```

### `src/models/macros.rs` — update three macros, add one

| Macro | Generates |
|---|---|
| `visitable_leaf!($T)` | empty `for_each_child` / `for_each_child_mut` |
| `visitable_forwarded!($T, $field)` | calls `f(&self.$field)` / `f(&mut self.$field)` |
| `visitable_forwarded_iter!($T, $field)` | iterates `$field`, calls `f` for each |
| `visitable_dispatch_enum!($T, $V1, $V2, ...)` | `match self { $T::$V(v) => f(v), ... }` — new |

### `src/models/json_schema.rs` — update `impl Visitable` blocks

| Type | Strategy | Rationale |
|---|---|---|
| `TypeBase` | `visitable_leaf!` | No schema children |
| `SchemaRootTypeBase` | `visitable_leaf!` | `defs` traversed by the containing root type |
| `StrEnumSchema` | `visitable_leaf!` | Enum values are strings, not schema nodes |
| All primitive schemas (`StringSchema`, `IntegerSchema`, `BooleanSchema`, `NullSchema`, `NumberSchema`, `DecimalSchema`, `ConstantSchema`, `AnySchema`, `ReferenceSchema`) | `visitable_leaf!` | No child schema nodes |
| `AnyOfSchema` | `visitable_forwarded_iter!(AnyOfSchema, any_of)` | Children are `any_of: Vec<SchemaType>` |
| `AllOfSchema` | `visitable_forwarded_iter!(AllOfSchema, all_of)` | Children are `all_of: Vec<SchemaType>` |
| `ArraySchema` | manual | Single child: `&*self.items` (`Box<SchemaType>`) |
| `ObjectSchema` | manual | Children are `properties.values()` |
| `SchemaRootObject` | manual | Children: `self.object` + `self.base.defs.values()` |
| `SchemaRootStrEnum` | manual | Children: `self.str_enum` + `self.base.defs.values()` |
| `SchemaType` | `visitable_dispatch_enum!` | Dispatch to inner variant |
| `SchemaClassType` | `visitable_dispatch_enum!` | Dispatch to inner variant |
| `SchemaRootType` | `visitable_dispatch_enum!` | Dispatch to inner variant |

Existing tests (`test_complex_root_object_visit_trait`, `test_complex_root_object_visit_and_mutate`) are updated to call `visit_all` / `visit_all_mut` instead of the removed `visit_by_type` / `visit_by_type_mut`.

### `src/edit/update_refs.rs` — adapt call site

`update_reference` is given an `Ok(())` stub body (logic implemented separately). The call site switches from the removed `visit_by_type_mut` to `try_visit_all_mut::<ReferenceSchema, Result<(), String>>`, passing a closure that returns `Result<(), String>` directly.

---

## Tests

All tests live in `visitable.rs` under `#[cfg(test)]`. A minimal three-level tree (no schema imports) is defined inside the test module:

```
Tree
├── Branch { tag: "left",  children: [Leaf(1), Leaf(2)] }
└── Branch { tag: "right", children: [Leaf(3)]          }
```

| Test | Verifies |
|---|---|
| `visit_all_visits_self_when_type_matches` | Self is included when its type matches `T` |
| `visit_all_collects_all_matching_nodes` | All `Leaf` nodes in a multi-level tree are found |
| `visit_all_only_visits_matching_type` | `Branch` count and `Leaf` count are independent |
| `visit_all_is_depth_first_preorder` | `Branch` nodes appear in left-then-right order |
| `visit_all_mut_modifies_all_matching_nodes` | All `Leaf` values are multiplied by 10 |
| `try_visit_all_mut_with_control_flow_stops_on_break` | `ControlFlow::Break(2)` after visiting `Leaf(1)` then `Leaf(2)` |
| `try_visit_all_mut_with_result_stops_on_err` | `Err("stop")` after visiting `Leaf(1)` then `Leaf(2)` |
| `try_visit_all_mut_returns_ok_when_no_error` | `Ok(())` when no closure returns `Err` |

---

## Usage Example

```rust
// Collect all $ref strings from a schema tree
let mut refs: HashSet<String> = HashSet::new();
(schema as &dyn Visitable).visit_all::<ReferenceSchema>(&mut |r| {
    refs.insert(r.r#ref.clone());
});

// Replace online references with relative paths.
// The closure returns Result<(), String> directly — no manual ControlFlow wrapping.
(schema as &mut dyn Visitable)
    .try_visit_all_mut::<ReferenceSchema, Result<(), String>>(&mut |r| rewrite_ref(r))?;

// Alternative using ControlFlow explicitly (also valid):
let result = (schema as &mut dyn Visitable)
    .try_visit_all_mut::<ReferenceSchema, ControlFlow<String>>(&mut |r| {
        if needs_rewrite(&r.r#ref) { ControlFlow::Break("bad ref".into()) }
        else { ControlFlow::Continue(()) }
    });
```
