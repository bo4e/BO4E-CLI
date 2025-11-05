use std::any::{Any, TypeId};
use std::fmt::Debug;
// pub trait Visitable: 'static {
//     fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable>>;
//     fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable>>;
//
//     fn visit_by_type<YieldType: 'static>(&self, f: &impl Fn(&YieldType))
//     where
//         Self: Sized,
//     {
//         let self_any: &dyn Any = &self;
//         if let Some(y) = self_any.downcast_ref::<YieldType>() {
//             f(y);
//         }
//         for sub in self.sub_nodes() {
//             sub.visit_by_type(f);
//         }
//     }
//     fn visit_by_type_mut<YieldType: 'static>(&mut self, f: &impl Fn(&mut YieldType))
//     where
//         Self: Sized,
//     {
//         let self_any: &mut dyn Any = &mut self;
//         if let Some(y) = self_any.downcast_mut::<YieldType>() {
//             f(y);
//         }
//         for sub in self.sub_nodes_mut() {
//             sub.visit_by_type_mut(f);
//         }
//     }
// }

pub trait Visitable: Any + Debug {
    fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_>;
    fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_>;

    // fn visit(&self, f: &dyn Fn(&dyn Visitable)) {
    //     f(self);
    //     for sub in self.sub_nodes() {
    //         sub.visit(f);
    //     }
    // }
    // fn visit_by_type_mut<YieldType: 'static>(&mut self, f: &impl Fn(&mut YieldType))
    // where
    //     Self: Sized,
    // {
    //     let self_any: &mut dyn Any = &mut self;
    //     if let Some(y) = self_any.downcast_mut::<YieldType>() {
    //         f(y);
    //     }
    //     for sub in self.sub_nodes_mut() {
    //         sub.visit_by_type_mut(f);
    //     }
    // }
}

impl dyn Visitable {
    pub fn visit_by_type<T: Visitable + Debug, F: Fn(&T)>(&self, f: &F) {
        let self_any: &dyn Any = self;
        // let self_type_id = format!("{:?}", self.type_id());
        // let self_debug_string = format!("{:?}", self);
        // let generic_type_name = format!("{:?}", std::any::type_name::<T>());
        // let generic_type_id = format!("{:?}", TypeId::of::<T>());
        // println!(
        //     "self.type_id() = {}, T.type_id() = {}",
        //     self_type_id, generic_type_id
        // );
        // println!("self = {}", self_debug_string);
        // println!("T = {}", generic_type_name);
        if let Some(y) = self_any.downcast_ref::<T>() {
            f(y);
        }
        for sub in self.sub_nodes() {
            // let sub_debug_string = format!("{:?}", sub);
            // println!("sub = {}", sub_debug_string);
            sub.visit_by_type(f);
        }
    }
    pub fn visit_by_type_mut<T: Visitable + Debug, F: Fn(&mut T)>(&mut self, f: &mut F) {
        let self_any: &mut dyn Any = self;
        if let Some(y) = self_any.downcast_mut::<T>() {
            f(y);
        }
        for sub in self.sub_nodes_mut() {
            sub.visit_by_type_mut(f);
        }
    }
}
