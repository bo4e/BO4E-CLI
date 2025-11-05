use crate::models::json_schema::{ObjectSchema, SchemaType};
use std::iter::FusedIterator;

pub trait Traversable<YieldType: 'static>
where
    Self: 'static,
{
    fn traverse(&self) -> TraverseIterator<YieldType>
    where
        Self: Sized,
    {
        TraverseIterator::new(self)
    }
    fn traverse_mut(&mut self) -> TraverseIteratorMut<YieldType>
    where
        Self: Sized,
    {
        TraverseIteratorMut::new(self)
    }
    fn yield_self(&self) -> Option<&YieldType> {
        if std::any::TypeId::of::<Self>() == std::any::TypeId::of::<YieldType>() {
            // SAFETY: We just checked that Self and YieldType are the same type
            Some(unsafe { &*(self as *const Self as *const YieldType) })
        } else {
            None
        }
    }
    fn yield_self_mut(&mut self) -> Option<&mut YieldType> {
        if std::any::TypeId::of::<Self>() == std::any::TypeId::of::<YieldType>() {
            // SAFETY: We just checked that Self and YieldType are the same type
            Some(unsafe { &mut *(self as *mut Self as *mut YieldType) })
        } else {
            None
        }
    }
    fn sub_nodes(&self) -> Box<dyn DoubleEndedIterator<Item = &dyn Traversable<YieldType>> + '_>;
    fn sub_nodes_mut(
        &mut self,
    ) -> Box<dyn DoubleEndedIterator<Item = &mut dyn Traversable<YieldType>> + '_>;
}

impl<T: 'static> Traversable<T> for ObjectSchema {
    fn sub_nodes(&self) -> Box<dyn DoubleEndedIterator<Item = &dyn Traversable<T>> + '_> {
        Box::new(
            self.properties
                .values()
                .map(|schema| schema as &dyn Traversable<T>)
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }
    fn sub_nodes_mut(
        &mut self,
    ) -> Box<dyn DoubleEndedIterator<Item = &mut dyn Traversable<T>> + '_> {
        Box::new(
            self.properties
                .values_mut()
                .map(|schema| schema as &mut dyn Traversable<T>)
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }
}

impl<T: 'static> Traversable<T> for SchemaType {}

pub struct TraverseIterator<'a, YieldT: 'static> {
    exploration_stack: Vec<&'a dyn Traversable<YieldT>>,
}

impl<'a, YieldT: 'static> TraverseIterator<'a, YieldT> {
    fn new(root: &'a dyn Traversable<YieldT>) -> Self {
        Self {
            exploration_stack: vec![root],
        }
    }
}

impl<'a, YieldT: 'static> Iterator for TraverseIterator<'a, YieldT> {
    type Item = &'a YieldT;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current_node) = self.exploration_stack.pop() {
            self.exploration_stack
                .extend(current_node.sub_nodes().rev());
            if let Some(yield_node) = current_node.yield_self() {
                return Some(yield_node);
            }
        }

        None
    }
}

impl<'a, YieldT: 'static> FusedIterator for TraverseIterator<'a, YieldT> {}

pub struct TraverseIteratorMut<'a, YieldT: 'static> {
    exploration_stack: Vec<&'a mut dyn Traversable<YieldT>>,
}

impl<'a, YieldT: 'static> TraverseIteratorMut<'a, YieldT> {
    fn new(root: &'a mut dyn Traversable<YieldT>) -> Self {
        Self {
            exploration_stack: vec![root],
        }
    }
}

impl<'a, YieldT: 'static> Iterator for TraverseIteratorMut<'a, YieldT> {
    type Item = &'a mut YieldT;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current_node) = self.exploration_stack.pop() {
            self.exploration_stack
                .extend(current_node.sub_nodes_mut().rev());
            return Some(unsafe {
                &mut *(current_node as *mut dyn Traversable<YieldT> as *mut YieldT)
            });
        }

        None
    }
}
