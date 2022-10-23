# System design models in rust.

Build queue models of distributed systems.

Inspired by lethain systems thinking toolkit https://github.com/lethain/systems
but targeted at distributed systems properties, i.e. queues / requests / responses / servers / loadbalancers / etc. And also in rust.

Developer creates a description of the system in rust dsl, then scheduler simulates it for a time interval, collecting metrics.

Dsl looks conceptually like this:

```
ArrivalSource { 
	delay: Poisson(1µs), 
	sink: LoadBalancer{
		servers: [
			Server { delay:  Poisson(10µs), sink: out},
			Server { delay:  Poisson(10µs), sink: out},
			Server { delay:  Poisson(10µs), sink: out},
		]
	}
}
```

Currently only just raw simulation is planned - a scheduler maintains a binary heap of queued things to do and does them one at a time. While it should be possible to estimate system behaviour using queue theory, its out of scope for now.
