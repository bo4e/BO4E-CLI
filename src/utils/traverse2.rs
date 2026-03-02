use regex::Replacer;
use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;

pub trait Traversable: Any + Debug {
    fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Traversable> + '_>;
    fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Traversable> + '_>;

    fn traverse_mut(&mut self) -> TraverseIteratorMut
    where
        Self: Sized,
        // Note: This function cannot be called on `dyn Traversable` as it is not dynamically
        // dispatchable.
    {
        TraverseIteratorMut::new(self)
    }
}

pub struct TraverseIteratorMut<'a> {
    exploration_deque: VecDeque<&'a mut dyn Traversable>,
}

impl<'a> TraverseIteratorMut<'a> {
    fn new(root: &'a mut dyn Traversable) -> Self {
        Self {
            exploration_deque: VecDeque::from([root]),
        }
    }
}

impl<'a> Iterator for TraverseIteratorMut<'a> {
    type Item = &'a mut dyn Traversable;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current_node) = self.exploration_deque.pop_front() {
            self.exploration_deque.extend(current_node.sub_nodes_mut());
            return Some(current_node);
        }

        None
    }
}
