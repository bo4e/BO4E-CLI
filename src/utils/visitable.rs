use std::any::Any;
use std::fmt::Debug;
use std::ops::ControlFlow;

/// A trait for types that form a tree, traversable with closures.
///
/// By accepting closures rather than returning iterators this trait is object-safe
/// and avoids runtime borrow checking (RefCell) for mutable traversal.
pub trait Visitable: Any + Debug {
    /// Visit each direct child of this node.
    #[allow(dead_code)]
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable));
    /// Visit each direct child of this node with mutable access.
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable));
}

impl dyn Visitable {
    /// Visit every node in the subtree whose concrete type is `T`.
    /// Traversal is depth-first pre-order (self before children).
    #[allow(dead_code)]
    pub fn visit_all<T: Any>(&self, f: &mut dyn FnMut(&T)) {
        if let Some(t) = (self as &dyn Any).downcast_ref::<T>() {
            f(t);
        }
        self.for_each_child(&mut |child| child.visit_all(f));
    }

    /// Like `visit_all`, but the closure can stop traversal early by returning
    /// `ControlFlow::Break(b)`. Returns `ControlFlow::Continue(())` if the whole
    /// tree was visited, or `ControlFlow::Break(b)` from the first break encountered.
    #[allow(dead_code)]
    pub fn try_visit_all<T: Any, B>(
        &self,
        f: &mut dyn FnMut(&T) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        if let Some(t) = (self as &dyn Any).downcast_ref::<T>() {
            match f(t) {
                ControlFlow::Continue(()) => {}
                ControlFlow::Break(b) => return ControlFlow::Break(b),
            }
        }
        let mut break_value: Option<B> = None;
        self.for_each_child(&mut |child| {
            if break_value.is_none() {
                if let ControlFlow::Break(b) = child.try_visit_all::<T, B>(f) {
                    break_value = Some(b);
                }
            }
        });
        match break_value {
            Some(b) => ControlFlow::Break(b),
            None => ControlFlow::Continue(()),
        }
    }

    /// Mutably visit every node in the subtree whose concrete type is `T`.
    #[allow(dead_code)]
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

    /// Like `visit_all_mut`, but the closure can stop traversal early by returning
    /// `ControlFlow::Break(b)`. Returns `ControlFlow::Continue(())` if the whole
    /// tree was visited, or `ControlFlow::Break(b)` from the first break encountered.
    pub fn try_visit_all_mut<T: Any, B>(
        &mut self,
        f: &mut dyn FnMut(&mut T) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        {
            let self_any: &mut dyn Any = self;
            if let Some(t) = self_any.downcast_mut::<T>() {
                match f(t) {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(b) => return ControlFlow::Break(b),
                }
            }
        }
        let mut break_value: Option<B> = None;
        self.for_each_child_mut(&mut |child| {
            if break_value.is_none() {
                if let ControlFlow::Break(b) = child.try_visit_all_mut::<T, B>(f) {
                    break_value = Some(b);
                }
            }
        });
        match break_value {
            Some(b) => ControlFlow::Break(b),
            None => ControlFlow::Continue(()),
        }
    }
}

pub fn cntrl_to_result<T, B>(cntrl: ControlFlow<B, T>) -> Result<T, B> {
    match cntrl {
        ControlFlow::Continue(t) => Ok(t),
        ControlFlow::Break(b) => Err(b),
    }
}

pub fn result_to_cntrl<T, B>(res: Result<T, B>) -> ControlFlow<B, T> {
    match res {
        Ok(t) => ControlFlow::Continue(t),
        Err(b) => ControlFlow::Break(b),
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
            left: Branch {
                tag: "left",
                children: vec![Leaf(1), Leaf(2)],
            },
            right: Branch {
                tag: "right",
                children: vec![Leaf(3)],
            },
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
    fn try_visit_all_mut_stops_on_break() {
        let mut tree = make_tree();
        let mut visited = vec![];
        let result = ((&mut tree) as &mut dyn Visitable).try_visit_all_mut::<Leaf, i32>(&mut |l| {
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
    fn try_visit_all_mut_returns_continue_when_no_break() {
        let mut tree = make_tree();
        let result = ((&mut tree) as &mut dyn Visitable)
            .try_visit_all_mut::<Leaf, ()>(&mut |_| ControlFlow::Continue(()));
        assert_eq!(result, ControlFlow::Continue(()));
    }
}
