use crate::systems::System;
use crate::traits::{HasQueue, StatEmitter, SystemRef, WorldMember};

use crate::influxdbreporter::InfluxDbReporter;

pub struct World {
    systems: Vec<System>,
}

impl World {
    pub fn new() -> Self {
        World {
            systems: Vec::new(),
        }
    }
    pub fn add(&mut self, system: System, name: String) -> SystemRef {
        let sr = self.systems.len();
        self.systems.push(system);
        self.with_system(sr, |system, _world| system.add(sr, name));
        sr
    }
    pub fn with_system<R, F: FnOnce(&mut System, &mut World) -> R>(
        &mut self,
        system_ref: SystemRef,
        f: F,
    ) -> R {
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
        let nw = World { systems: nsystems };
        (unset, nw)
    }
}

impl HasQueue for World {
    fn queue_size(&self) -> i64 {
        let mut qs = 0;
        for system in &self.systems {
            qs += system.queue_size();
        }
        qs
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
    fn eq(&self, o: &Self) -> bool {
        self.t == o.t
    }
}
impl Eq for SchedulerElement {}
impl PartialOrd for SchedulerElement {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        self.t.partial_cmp(&o.t).map(|e| e.reverse())
    }
}
impl Ord for SchedulerElement {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.t.cmp(&o.t).reverse()
    }
}

use crate::influxdbreporter::SimulationReachedTimeEvent;
use crate::utils::Counter;
use std::collections::BinaryHeap;
use tokio::sync::mpsc;
pub struct Scheduler {
    heap: BinaryHeap<SchedulerElement>,
    cur_t_ns: i64,
    executed: Counter,
    event_tx: mpsc::Sender<SimulationReachedTimeEvent>,
    reported_cur_t_ns: Option<i64>,
}

impl Scheduler {
    pub fn new() -> Self {
        let binary_heap = BinaryHeap::<SchedulerElement>::new();
        let (tx, rx) = mpsc::channel::<SimulationReachedTimeEvent>(1_000_000);
        let reporter = InfluxDbReporter::new(rx);
        reporter.start();
        Scheduler {
            heap: binary_heap,
            cur_t_ns: 0,
            executed: Counter::new(),
            event_tx: tx,
            reported_cur_t_ns: None,
        }
    }

    pub fn schedule(&mut self, world: &mut World, emitter: SystemRef) {
        world.with_system(emitter, |system, world| {
            let nt = system.tick(self, world);
            if let Some(nt) = nt {
                self.schedule_at(nt, emitter);
            }
        });
    }

    pub fn schedule_at(&mut self, t: i64, emitter: SystemRef) {
        self.heap.push(SchedulerElement {
            t,
            e: EmitterRef { aref: emitter },
        });
    }

    pub fn execute_next(&mut self, world: &mut World, up_to_nano: i64) -> bool {
        let top = self.heap.pop();
        if let Some(top) = top {
            self.executed.inc();
            let ee = top.e;
            self.cur_t_ns = top.t;
            if self.cur_t_ns >= up_to_nano {
                self.reportmetrics(true);
                false
            } else {
                self.reportmetrics(false);
                let nt = world.with_system(ee.aref, |system, world| -> Option<i64> {
                    system.tick(self, world)
                });
                if let Some(nt) = nt {
                    self.heap.push(SchedulerElement { t: nt, e: ee });
                    true
                } else {
                    self.heap.len() > 0
                }
            }
        } else {
            false
        }
    }

    pub fn get_cur_t(&self) -> i64 {
        self.cur_t_ns
    }

    fn reportmetrics(&mut self, stop: bool) {
        if self.reported_cur_t_ns.is_none() ||
            self.cur_t_ns > self.reported_cur_t_ns.unwrap() + 5_000_000_00 {
            futures::executor::block_on(self.event_tx.send(SimulationReachedTimeEvent {
                time_ns: self.cur_t_ns,
                stop: stop,
            }))
            .unwrap();
            self.reported_cur_t_ns = Some(self.cur_t_ns);
        }
    }
}

impl StatEmitter for Scheduler {
    fn stats(&self) -> String {
        format!("executed {}", self.executed.stats())
    }
}
