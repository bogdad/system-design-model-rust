pub mod utils;

extern crate rand;

use utils::{tostring, Meter, Counter};

use rand::RngCore;
struct Context {
    rng: Box<dyn RngCore>,
}

use std::ops::Deref;
use std::rc::Rc;
use std::cell::RefCell;
struct EmitterRef {
    aref: Rc<RefCell<dyn Emmitter>>,
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

    fn schedule(&mut self, emitter: Rc<RefCell<dyn Emmitter>>) {
        let nt = emitter.deref().borrow_mut().tick(self).unwrap();
        self.heap.push(SchedulerElement { t: nt, e: EmitterRef{aref: emitter}});
    }

    fn execute_next(&mut self, up_to_nano: i64) -> bool {
        let top = self.heap.pop();
        if let Some(top) = top {
            self.executed += 1;
            let ee = top.e;
            self.cur_t = top.t;
            if self.cur_t > up_to_nano {
                false
            } else {
                let nt = ee.aref.borrow_mut().tick(self);
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


trait Emmitter {
    fn tick(&mut self, scheduler: &mut Scheduler) -> Option<i64>;
}

trait Sink {
    fn next(&mut self, scheduler: &mut Scheduler);
}


use rand_distr::Poisson;
use rand_distr::Distribution;


struct ArrivalSource {
    distribution: Poisson<f64>,
    sink: Rc<RefCell<dyn Sink>>,
    meter: Meter,
}

impl<'a> Emmitter for ArrivalSource {

    fn tick(&mut self, scheduler: &mut Scheduler) -> Option<i64> {
        let diff = (self.distribution.sample(&mut rand::thread_rng()) ) as i64;
        let next_time = scheduler.cur_t +  diff;
        self.meter.inc(diff);
        
        self.sink.borrow_mut().next(scheduler);
        Some(next_time as i64)
    }
}

impl ArrivalSource {
    fn print_stats(&self) {
        println!("as {}", self.meter.stats());
    }
}

struct EndSink {
    ticks: u32,
}

impl Sink for EndSink {

    fn next(&mut self, scheduler: &mut Scheduler) {
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
    sink: Rc<RefCell<dyn Sink>>,
    queue: VecDeque<i64>,
}

struct ServerRef {
    sc: Rc<RefCell<Server>>,
    meter: Meter,
    counter: Counter,
}



impl ServerRef {
    fn stats(&self) -> String {
        let ssc = self.sc.deref().borrow();
        format!("meter {} queue {} counter {}", self.meter.stats(), 
            tostring(ssc.queue.len()), self.counter.stats())
    }
}

impl<'a> Emmitter for Server {

    fn tick(&mut self, scheduler: &mut Scheduler) -> Option<i64> {
        let ob = self.queue.front().cloned();
        self.queue.pop_front();
        self.sink.borrow_mut().next(scheduler);
        let nb = self.queue.front();
        self.queue.front().cloned()
    }
}

impl Sink for ServerRef {
    fn next(&mut self, scheduler: &mut Scheduler) {
        let mut sc = self.sc.borrow_mut();
        let next_time = (sc.distribution.sample(&mut rand::thread_rng())) as i64;
        if sc.queue.is_empty() {
            let nt = scheduler.cur_t + next_time;
            sc.queue.push_back(nt);

            scheduler.heap.push(SchedulerElement {
                        t: nt,
                        e: EmitterRef{aref: self.sc.clone()}
                    });
        } else {
            let top = sc.queue.back();
            let nt = top.unwrap() + next_time;
            sc.queue.push_back(nt);
        }
        self.meter.inc(next_time);
        self.counter.inc();
    }
}

fn main() {

    let end_sink = Rc::new(RefCell::new(EndSink{ticks: 0}));
    let up_to_nano = 1_000_000_000;
    
    let server_ref = Rc::new(RefCell::new(ServerRef {
        sc: Rc::new(RefCell::new(Server {
            distribution: Poisson::<f64>::new(20_000.0).unwrap(),
            sink: end_sink.clone(),
            queue: VecDeque::new(),
        })),
        meter: Meter::new(),
        counter: Counter::new(),
    }));

    let ar: Rc<RefCell<ArrivalSource>> = 
    Rc::new(RefCell::new(
        ArrivalSource{ 
            distribution: Poisson::<f64>::new(1_000.0).unwrap(), 
            sink: server_ref.clone(), 
            meter: Meter::new()}));
    
    let binary_heap = BinaryHeap::<SchedulerElement>::new();
    let mut scheduler = Scheduler {
        heap: binary_heap,
        cur_t: 0,
        executed: 0,
    };

    scheduler.schedule(ar.clone());

    while scheduler.execute_next(up_to_nano) {

    }
    println!("executed {}", tostring(scheduler.executed));
    
    
    ar.deref().borrow().print_stats();
    println!("server {}", server_ref.deref().borrow().stats());
    println!("endsink {}", tostring(end_sink.deref().borrow().ticks));
}
