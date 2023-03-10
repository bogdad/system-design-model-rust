pub type SystemRef = usize;

use crate::objects::Scheduler;
use crate::objects::World;

pub trait WorldMember {
    fn add(&mut self, system_ref: SystemRef, name: String);
    fn getref(&self) -> Option<SystemRef>;
}

pub trait Emmitter {
    fn tick(&mut self, world: &mut World, scheduler: &mut Scheduler) -> Option<i64>;
}

pub trait Sink {
    fn next(&mut self, world: &mut World, scheduler: &mut Scheduler);
}

pub trait StatEmitter {
    fn stats(&self) -> String;
}

pub trait HasQueue {
    fn queue_size(&self) -> i64;
}
