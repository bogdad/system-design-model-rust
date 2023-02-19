
use crate::objects::{Scheduler, World};
use crate::traits::{SystemRef, StatEmitter, Emmitter, WorldMember, Sink, HasQueue};
use crate::utils::{Counter, Meter, tostring};

use rand_distr::Distribution;
use rand_distr::Poisson;

pub struct ArrivalSource {
    distribution: Poisson<f64>,
    sink: SystemRef,
    meter: Meter,
    sr: Option<SystemRef>,
}

impl ArrivalSource {
    pub fn new(distribution: Poisson<f64>, sink: SystemRef) -> Self {
        ArrivalSource { distribution, sink, meter: Meter::new(), sr: None}
    }
    
}


pub struct EndSink {
    ticks: Counter,
    sr: Option<SystemRef>,
}

impl EndSink {
    pub fn new() -> Self {
        EndSink {ticks: Counter::new(), sr: None}
    }
}




impl StatEmitter for ArrivalSource {
    fn stats(&self) -> String {
        format!("as {}", self.meter.stats())
    }
}

impl WorldMember for ArrivalSource {
    fn add(&mut self, system_ref: SystemRef, name: String) {
        self.meter.name = Some(name + "_meter");
        self.sr = Some(system_ref)
    }

    fn getref(&self) -> Option<SystemRef> {
        self.sr
    }
}

impl<'a> Emmitter for ArrivalSource {

    fn tick(&mut self, world: &mut World, scheduler: &mut Scheduler) -> Option<i64> {
        let diff = (self.distribution.sample(&mut rand::thread_rng()) ) as i64;
        let next_time = scheduler.get_cur_t() +  diff;
        self.meter.inc(diff);

        world.with_system(self.sink, |system, world|{
            system.next(world, scheduler);
        });
        Some(next_time as i64)
    }
}

impl StatEmitter for EndSink {
    fn stats(&self) -> String {
        format!("processed {}", self.ticks.stats())
    }
}

impl WorldMember for EndSink {
    fn add(&mut self, system_ref: SystemRef, name: String) {
        self.ticks.name = Some(name + "_ticks");
        self.sr = Some(system_ref)
    }

    fn getref(&self) -> Option<SystemRef> {
        self.sr
    }
}

impl Sink for EndSink {

    fn next(&mut self, _world: &mut World, _scheduler: &mut Scheduler) {
        self.ticks.inc()
    }
}

/// Loadbalancer distributes incoming requests across a series of sinks.
/// Currenly it does not have a queue of its own.
pub struct LoadBalancer {
    sinks: Vec<SystemRef>,
    sr: Option<SystemRef>,
    counter: Counter,
    cur: usize,
}

impl LoadBalancer {
    pub fn new(sinks: Vec<SystemRef>) -> Self {
        assert!(sinks.len() > 0);
        LoadBalancer { sinks, sr: None, counter: Counter::new(), cur: 0 }
    }
}

impl WorldMember for LoadBalancer {
    fn add(&mut self, system_ref: SystemRef, name: String) {
        self.counter.name = Some(name + "_counter");
        self.sr = Some(system_ref)
    }

    fn getref(&self) -> Option<SystemRef> {
        self.sr
    }
}

impl Sink for LoadBalancer {
    fn next(&mut self, world: &mut World, scheduler: &mut Scheduler) {
        let next_sink_ref = self.sinks[self.cur];
        world.with_system(next_sink_ref, |system, world| {
            system.next(world, scheduler);
        });
        self.cur += 1;
        self.cur %= self.sinks.len();
        self.counter.inc();
    }
}

impl StatEmitter for LoadBalancer {
    fn stats(&self) -> String {
        format!("lb incoming {}", self.counter.stats())
    }
}


use std::collections::VecDeque;
pub struct Server {
    distribution: Poisson<f64>,
    sink: SystemRef,
    queue: VecDeque<i64>,
    meter: Meter,
    counter: Counter,
    sr: Option<SystemRef>,
}

impl Server {
    pub fn new(distribution: Poisson<f64>, sink: SystemRef) -> Self {
        Server {
            distribution,
            sink,
            queue: VecDeque::new(),
            meter: Meter::new(),
            counter: Counter::new(),
            sr: None,
        }
    }
}

impl Sink for Server {
    fn next(&mut self, _world: &mut World, scheduler: &mut Scheduler) {
        let next_time = (self.distribution.sample(&mut rand::thread_rng())) as i64;
        if self.queue.is_empty() {
            let nt = scheduler.get_cur_t() + next_time;
            self.queue.push_back(nt);

            scheduler.schedule_at(nt, self.getref().unwrap());
        } else {
            let top = self.queue.back();
            let nt = top.unwrap() + next_time;
            self.queue.push_back(nt);
        }
        self.meter.inc(next_time);
        self.counter.inc();
    }
}

impl StatEmitter for Server {
    fn stats(&self) -> String {
        format!("meter {} queue {} counter {}", self.meter.stats(), 
            tostring(self.queue.len()), self.counter.stats())
    }
}

impl HasQueue for Server {
    fn queue_size(&self) -> i64 {
        self.queue.len() as i64
    }
}


impl WorldMember for Server {
    fn add(&mut self, system_ref: SystemRef, name: String) {
        self.meter.name = Some(name + "_meter");
        self.sr = Some(system_ref);
    }

    fn getref(&self) -> Option<SystemRef> {
        self.sr
    }
}

impl<'a> Emmitter for Server {

    fn tick(&mut self, world: &mut World, scheduler: &mut Scheduler) -> Option<i64> {
        let _ob = self.queue.front().cloned();
        self.queue.pop_front();
        world.with_system(self.sink, |system, world|{system.next(world, scheduler)});
        let _nb = self.queue.front();
        self.queue.front().cloned()
    }
}


pub enum System {
    Unset,
    EndSink(EndSink),
    Server(Server),
    ArrivalSource(ArrivalSource),
    LoadBalancer(LoadBalancer),
}

impl System {
    pub fn tick(&mut self, scheduler: &mut Scheduler, world: &mut World) -> Option<i64> {
        match self {
            System::EndSink(_) => unimplemented!(),
            System::Server(server) => server.tick(world, scheduler),
            System::ArrivalSource(ars) => ars.tick(world, scheduler),
            System::Unset => unimplemented!(),
            System::LoadBalancer(_lb) => unimplemented!(),
        }
    }

    pub fn next(&mut self, world: &mut World, scheduler: &mut Scheduler) {
            match self {
                System::EndSink(es) => es.next(world, scheduler),
                System::Server(sr) => sr.next(world, scheduler),
                System::ArrivalSource(_ars) => unimplemented!(),
                System::Unset => unimplemented!(),
                System::LoadBalancer(lb) => lb.next(world, scheduler),
            }
    }

    pub fn queue_size(&self) -> i64 {
        match self {
            System::Unset => 0,
            System::EndSink(_) => 0,
            System::Server(s) => s.queue_size(),
            System::ArrivalSource(_) => 0,
            System::LoadBalancer(_) => 0,
        }
    }
}

impl StatEmitter for System {
	fn stats(&self) -> String {
        match self {
            System::EndSink(es) => es.stats(),
            System::Server(sr) => sr.stats(),
            System::ArrivalSource(asr) => asr.stats(),
            System::Unset => unimplemented!(),
            System::LoadBalancer(lb) => lb.stats(),
        }
    }
}

impl WorldMember for System {
    fn add(&mut self, system_ref: SystemRef, name: String) {
        match self {
            System::EndSink(endsink) => endsink.add(system_ref, name),
            System::Server(sv) => sv.add(system_ref, name),
            System::ArrivalSource(arrival_source) => arrival_source.add(system_ref, name),
            System::Unset => unimplemented!(),
            System::LoadBalancer(lb) => lb.add(system_ref, name),
        }
    }

    fn getref(&self) -> Option<SystemRef> {
        match self {
            System::EndSink(es) => es.getref(),
            System::Server(_) => todo!(),
            System::ArrivalSource(ars) => ars.getref(),
            System::Unset => unimplemented!(),
            System::LoadBalancer(lb) => lb.getref(),
        }
    }
}

