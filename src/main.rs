pub mod utils;

extern crate rand;

use utils::{tostring, Meter, Counter};


trait WorldMember {
    fn add(&mut self, system_ref: SystemRef);
    fn getref(&self) -> Option<SystemRef>;
}

trait Emmitter {
    fn tick(&mut self, world: &mut World, scheduler: &mut Scheduler) -> Option<i64>;
}

trait Sink {
    fn next(&mut self, scheduler: &mut Scheduler);
}

trait StatEmitter {
    fn stats(&self) -> String;
}

use rand_distr::Poisson;
use rand_distr::Distribution;


struct ArrivalSource {
    distribution: Poisson<f64>,
    sink: SystemRef,
    meter: Meter,
    sr: Option<SystemRef>,
}

impl ArrivalSource {
    fn new(distribution: Poisson<f64>, sink: SystemRef) -> Self {
        ArrivalSource { distribution, sink, meter: Meter::new(), sr: None }
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
        let next_time = scheduler.cur_t +  diff;
        self.meter.inc(diff);

        world.with_system(self.sink, |system, _world|{
            system.next(scheduler);
        });
        Some(next_time as i64)
    }
}

struct EndSink {
    ticks: u32,
    sr: Option<SystemRef>,
}

impl EndSink {
    fn new() -> Self {
        EndSink {ticks: 0, sr: None}
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

struct Server {
    distribution: Poisson<f64>,
    sink: SystemRef,
    queue: VecDeque<i64>,
    meter: Meter,
    counter: Counter,
    sr: Option<SystemRef>,
}

impl Server {
    fn new(distribution: Poisson<f64>, sink: SystemRef) -> Self {
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
            let nt = scheduler.cur_t + next_time;
            self.queue.push_back(nt);

            scheduler.heap.push(SchedulerElement {
                        t: nt,
                        e: EmitterRef{aref: self.getref().unwrap()}
                    });
        } else {
            let top = self.queue.back();
            let nt = top.unwrap() + next_time;
            self.queue.push_back(nt);
        }
        self.meter.inc(next_time);
        self.counter.inc();
    }
}



enum System {
    Unset,
    EndSink(EndSink),
    Server(Server),
    ArrivalSource(ArrivalSource),
}

impl System {
    fn tick(&mut self, scheduler: &mut Scheduler, world: &mut World) -> Option<i64> {
        match self {
            System::EndSink(_) => unimplemented!(),
            System::Server(server) => server.tick(world, scheduler),
            System::ArrivalSource(ars) => ars.tick(world, scheduler),
            System::Unset => unimplemented!(),
        }
    }

    fn next(&mut self, scheduler: &mut Scheduler) {
            match self {
                System::EndSink(es) => es.next(scheduler),
                System::Server(sr) => sr.next(scheduler),
                System::ArrivalSource(_ars) => unimplemented!(),
                System::Unset => unimplemented!(),
            }
    }

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

type SystemRef = usize;

struct World {
    systems: Vec<System>,
}

impl World {
    fn new() -> Self {
        World { systems: Vec::new()}
    }
    fn add(&mut self, system: System) -> SystemRef {
        let sr = self.systems.len();
        self.systems.push(system);
        self.with_system(sr, |system, _world|{
            system.add(sr)
        });
        sr
    }
    fn with_system<R, F: FnMut(&mut System, &mut World) -> R>(&mut self, system_ref: SystemRef, mut f:F) -> R {
        let (mut s, mut nw) = self.split(system_ref);
        let r = f(&mut s, &mut nw);
        std::mem::swap(self, &mut nw);
        self.systems[system_ref] = s;
        r
    }

    fn split(&mut self, system_ref: SystemRef) -> (System, World) {
        let mut unset = System::Unset;
        std::mem::swap(&mut unset, self.systems.get_mut(system_ref).unwrap());
        let mut nsystems = vec![];
        std::mem::swap(&mut nsystems, &mut self.systems);
        let nw = World {
            systems: nsystems
        };
        (unset, nw)
    }
}

struct EmitterRef {
    aref: SystemRef,
}

struct SchedulerElement {
    t: i64,
    e: EmitterRef,
}
impl PartialEq for SchedulerElement {
fn eq(&self, o: &Self) -> bool { self.t == o.t }
}
impl Eq for SchedulerElement {
}
impl PartialOrd for SchedulerElement {
fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { self.t.partial_cmp(&o.t).map(|e|{e.reverse()})}
}
impl Ord for SchedulerElement {

fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.t.cmp(&o.t).reverse()}
}

use std::collections::BinaryHeap;
use std::collections::VecDeque;
struct Scheduler {
    heap: BinaryHeap<SchedulerElement>,
    cur_t: i64,
    executed: i64,
}

impl Scheduler {

    fn schedule(&mut self, world: &mut World, emitter: SystemRef) {
        world.with_system(emitter, |system, world|{
            let nt = system.tick(self, world);
            if let Some(nt) = nt {
                self.heap.push(SchedulerElement { t: nt, e: EmitterRef{aref: emitter}});
            }
        });
    }

    fn execute_next(&mut self, world: &mut World, up_to_nano: i64) -> bool {
        let top = self.heap.pop();
        if let Some(top) = top {
            self.executed += 1;
            let ee = top.e;
            self.cur_t = top.t;
            if self.cur_t > up_to_nano {
                false
            } else {
                let nt = world.with_system(ee.aref, |system, world| -> Option<i64> {
                    system.tick(self, world)
                });
                if let Some(nt) = nt {
                    self.heap.push(SchedulerElement {
                        t: nt,
                        e: ee
                    });
                    true
                } else {
                    self.heap.len() > 0
                }
            }
        } else {
            false
        }
    }
}


fn main() {

    let mut world = World::new();

    let endsink = EndSink::new();
    let endsink_ref = world.add(System::EndSink(endsink));

    let up_to_nano = 1_000_000_000;
    
    let server = Server::new(Poisson::<f64>::new(20_000.0).unwrap(), endsink_ref);
    let server_ref = world.add(System::Server(server));

    let ar = ArrivalSource::new( Poisson::<f64>::new(1_000.0).unwrap(), server_ref);
    let ar_ref = world.add(System::ArrivalSource(ar));
    
    let binary_heap = BinaryHeap::<SchedulerElement>::new();
    let mut scheduler = Scheduler {
        heap: binary_heap,
        cur_t: 0,
        executed: 0,
    };

    scheduler.schedule(&mut world, ar_ref);

    while scheduler.execute_next(&mut world, up_to_nano) {

    }
    println!("executed {}", tostring(scheduler.executed));
    
    
    world.with_system(ar_ref, |ar, _w|{println!("ar {}", ar.stats());});
    world.with_system(server_ref, |server, _w| println!("server {}", server.stats()) );
    world.with_system(endsink_ref, |endsink, _w| println!("endsink {}", endsink.stats()) );
}
