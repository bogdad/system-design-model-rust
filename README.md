# System design models in rust.

Simulating queue theory models of distributed systems.

Inspired by lethain systems thinking toolkit https://github.com/lethain/systems
but targeted at distributed systems properties, i.e. queues / requests / responses / servers / loadbalancers / etc. And also in rust.

Create a description of the distributed system in rust dsl, use scheduler to simulate passage of time, record metrics.
Then compare simulated results to actual prometheus/grafana to test which assumptions do not hold.

Dsl looks conceptually like this:

```
ArrivalSource { 
	delay: Poisson(1µs), 
	sink: LoadBalancer {
		servers: [
			Server { delay:  Poisson(10µs), sink: out},
			Server { delay:  Poisson(10µs), sink: out},
			Server { delay:  Poisson(10µs), sink: out},
		]
	}
}
```

Currently only just raw simulation is planned - a scheduler maintains a binary heap of queued things to do and does them one at a time. While it should be possible to estimate system behaviour using queue theory, its out of scope for now.
