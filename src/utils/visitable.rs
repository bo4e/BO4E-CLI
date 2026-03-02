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
