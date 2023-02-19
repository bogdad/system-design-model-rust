pub mod utils;
pub mod systems;
pub mod objects;
pub mod traits;
pub mod influxdbreporter;

#[macro_use]
extern crate derive_builder;
extern crate rand;

use crate::objects::{World, Scheduler};
use crate::systems::{ArrivalSource, Server, EndSink, LoadBalancer, System};
use crate::traits::{StatEmitter, HasQueue};
use crate::utils::tostring;

use rand_distr::Poisson;

fn main() {

    let mut world = World::new();

    let endsink = EndSink::new();
    let endsink_ref = world.add(System::EndSink(endsink), "endsink".to_string());

    let up_to_nano = 1_000_000_000;
    
    let server1 = Server::new(Poisson::<f64>::new(20_000.0).unwrap(), endsink_ref);
    let server1_ref = world.add(System::Server(server1), "server1".to_string());

    let server2 = Server::new(Poisson::<f64>::new(20_000.0).unwrap(), endsink_ref);
    let server2_ref = world.add(System::Server(server2), "server2".to_string());

    let load_balancer = LoadBalancer::new(vec![server1_ref, server2_ref]);
    let load_balancer_ref = world.add(System::LoadBalancer(load_balancer), "load_balancer".to_string());

    let ar = ArrivalSource::new( Poisson::<f64>::new(1_000.0).unwrap(), load_balancer_ref);
    let ar_ref = world.add(System::ArrivalSource(ar), "incomming".to_string());
    
    let mut scheduler = Scheduler::new();

    scheduler.schedule(&mut world, ar_ref);

    while scheduler.execute_next(&mut world, up_to_nano) {

    }
    println!("executed {}", scheduler.stats());
    
    
    world.with_system(ar_ref, |ar, _w|{println!("ar {}", ar.stats());});
    world.with_system(server1_ref, |server, _w| println!("server1 {}", server.stats()) );
    world.with_system(server2_ref, |server, _w| println!("server2 {}", server.stats()) );
    world.with_system(load_balancer_ref, |load_balancer, _w| println!("load balaner {}", load_balancer.stats()) );
    world.with_system(endsink_ref, |endsink, _w| println!("endsink {}", endsink.stats()) );

    println!("requests in the system {}", tostring(world.queue_size()));
}
