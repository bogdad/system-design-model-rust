
use crate::objects::{Scheduler, World};
use crate::traits::{SystemRef, StatEmitter, Emmitter, WorldMember, Sink};
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
        ArrivalSource { distribution, sink, meter: Meter::new(), sr: None }
    }
    
}


pub struct EndSink {
    ticks: u32,
    sr: Option<SystemRef>,
}

impl EndSink {
    pub fn new() -> Self {
        EndSink {ticks: 0, sr: None}
    }
}




impl StatEmitter for ArrivalSource {
    fn stats(&self) -> String {
        format!("as {}", self.meter.stats())
    }
}

impl WorldMember for ArrivalSource {
    fn add(&mut self, system_ref: SystemRef) {
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

        world.with_system(self.sink, |system, _world|{
            system.next(scheduler);
        });
        Some(next_time as i64)
    }
}

impl StatEmitter for EndSink {
    fn stats(&self) -> String {
        tostring(self.ticks)
    }
}

impl WorldMember for EndSink {
    fn add(&mut self, system_ref: SystemRef) {
        self.sr = Some(system_ref);
    }

    fn getref(&self) -> Option<SystemRef> {
        self.sr
    }
}

impl Sink for EndSink {

    fn next(&mut self, _scheduler: &mut Scheduler) {
        self.ticks += 1;
    }
}


impl WorldMember for Server {
    fn add(&mut self, system_ref: SystemRef) {
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
        world.with_system(self.sink, |system, _world|{system.next(scheduler)});
        let _nb = self.queue.front();
        self.queue.front().cloned()
    }
}

impl Sink for Server {
    fn next(&mut self, scheduler: &mut Scheduler) {
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

/*struct LoadBalancer {
    source: Box<dyn Source>,
    sinks: Vec<Box<dyn Sink>>,
}

impl Emmitter for LoadBalancer {
    fn next_time(&mut self) -> Option<i64> { todo!() }
    fn tick(&mut self, _: &mut Scheduler<'_>) { todo!() }
}
impl Source for LoadBalancer {
}
*/

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
impl StatEmitter for Server {
    fn stats(&self) -> String {
        format!("meter {} queue {} counter {}", self.meter.stats(), 
            tostring(self.queue.len()), self.counter.stats())
    }
}



pub enum System {
    Unset,
    EndSink(EndSink),
    Server(Server),
    ArrivalSource(ArrivalSource),
}

impl System {
    pub fn tick(&mut self, scheduler: &mut Scheduler, world: &mut World) -> Option<i64> {
        match self {
            System::EndSink(_) => unimplemented!(),
            System::Server(server) => server.tick(world, scheduler),
            System::ArrivalSource(ars) => ars.tick(world, scheduler),
            System::Unset => unimplemented!(),
        }
    }

    pub fn next(&mut self, scheduler: &mut Scheduler) {
            match self {
                System::EndSink(es) => es.next(scheduler),
                System::Server(sr) => sr.next(scheduler),
                System::ArrivalSource(_ars) => unimplemented!(),
                System::Unset => unimplemented!(),
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
        }
    }
}

impl WorldMember for System {
    fn add(&mut self, system_ref: SystemRef) {
        match self {
            System::EndSink(endsink) => endsink.add(system_ref),
            System::Server(sv) => sv.add(system_ref),
            System::ArrivalSource(arrival_source) => arrival_source.add(system_ref),
            System::Unset => unimplemented!(),
        }
    }

    fn getref(&self) -> Option<SystemRef> {
        match self {
            System::EndSink(es) => es.getref(),
            System::Server(_) => todo!(),
            System::ArrivalSource(ars) => ars.getref(),
            System::Unset => unimplemented!(),
        }
    }
}
