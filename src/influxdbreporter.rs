use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use influxdb::{Client, InfluxDbWriteable, Timestamp, WriteQuery};

//use log::warn;
use metriki_core::metrics::*;
use metriki_core::MetricsRegistry;
use tokio::sync::mpsc;
use tokio::time::Duration;

use metriki_core::global::global_registry;
use std::env;

/*
this is almost a copy of
https://github.com/sunng87/metriki/blob/master/metriki-influxdb-reporter/src/lib.rs
*/

#[derive(Debug)]
pub struct SimulationReachedTimeEvent {
    pub time_ns: i64,
    pub stop: bool,
}

#[derive(Debug)]
pub struct InfluxDbReporter {
    registry: Arc<MetricsRegistry>,
    url: String,
    database: String,
    username: Option<String>,
    password: Option<String>,
    auth_token: Option<String>,
    measurement_prefix: String,
    tags: HashMap<String, String>,
    batch_size: usize,
    event_rx: mpsc::Receiver<SimulationReachedTimeEvent>,
    last_send_time_ns: Option<i64>,
    test_start_time: SystemTime,
    send_interval_ns: i64,
}

impl InfluxDbReporter {
    pub fn new(event_rx: mpsc::Receiver<SimulationReachedTimeEvent>) -> Self {
        use std::ops::Sub;
        let hour_secs = 60 * 60;
        let interval_secs = 1;
        let mut tags = HashMap::new();
        let test_start_time = SystemTime::now().sub(Duration::from_secs(hour_secs));
        let test_start_time_string: String = test_start_time
            .duration_since(UNIX_EPOCH)
            .ok()
            .unwrap()
            .as_secs()
            .to_string();
        tags.insert("test_start_time".to_string(), test_start_time_string);
        InfluxDbReporter {
            registry: global_registry(),
            url: "http://localhost:8086".to_string(),
            database: "grafana".to_string(),
            username: None,
            password: None,
            auth_token: env::var("AUTH_TOKEN").ok(),
            measurement_prefix: "sdm_".to_string(),
            tags: tags,
            batch_size: 60,
            event_rx: event_rx,
            last_send_time_ns: None,
            test_start_time: test_start_time,
            send_interval_ns: interval_secs * 1_000_000_000,
        }
    }

    fn new_client(&self) -> Client {
        let client = Client::new(&self.url, &self.database);
        if let (Some(username), Some(password)) = (self.username.as_ref(), self.password.as_ref()) {
            client.with_auth(username, password)
        } else if let Some(auth_token) = &self.auth_token {
            client.with_token(auth_token)
        } else {
            client
        }
    }

    pub fn start(mut self) {
        let test_start_time_string: String = self.test_start_time
            .duration_since(UNIX_EPOCH)
            .ok()
            .unwrap()
            .as_secs()
            .to_string();
        println!(
            "influxdb: test_start_time {}", test_start_time_string
        );
        let looper = move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("can not create tokio runtime");
            runtime.block_on(async {
                let mut need_stop = false;
                while !need_stop {
                    while let Some(event) = self.event_rx.recv().await {
                        if event.stop {
                            need_stop = true;
                        }
                        // TODO: fix, check if interval from last reporting is larger than send interval
                        if self.last_send_time_ns.is_none()
                            || self.last_send_time_ns.unwrap() + self.send_interval_ns
                                <= event.time_ns
                        {
                            let metrics = self.registry.snapshots();
                            let client = self.new_client();
                            let queries: Vec<WriteQuery> = metrics
                                .iter()
                                .map(|(key, metric)| match metric {
                                    Metric::Counter(c) => {
                                        self.report_counter(key, c.as_ref(), event.time_ns)
                                    }
                                    Metric::Gauge(g) => {
                                        self.report_gauge(key, g.as_ref(), event.time_ns)
                                    }
                                    Metric::Timer(t) => {
                                        self.report_timer(key, t.as_ref(), event.time_ns)
                                    }
                                    Metric::Meter(m) => {
                                        self.report_meter(key, m.as_ref(), event.time_ns)
                                    }
                                    Metric::Histogram(h) => {
                                        self.report_histogram(key, &h.snapshot(), event.time_ns)
                                    }
                                })
                                .collect();

                            if !queries.is_empty() {
                                self.do_query(&client, queries).await;
                            }
                            self.last_send_time_ns = Some(event.time_ns);
                        }
                    }
                }
                let test_start_time_string: String = self.test_start_time
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .unwrap()
                    .as_secs()
                    .to_string();
                println!(
                    "influxdb: test_start_time {}", test_start_time_string
                );
            })
        };
        std::thread::spawn(looper);
    }

    #[inline]
    fn measurement(&self, name: &str) -> String {
        format!("{}{}", self.measurement_prefix, name)
    }

    #[inline]
    fn with_query(&self, name: &str, since_start: i64) -> WriteQuery {
        let mut query = Timestamp::Milliseconds(self.system_time_millis(since_start))
            .into_query(self.measurement(name));

        for (k, v) in self.tags.iter() {
            query = query.add_tag(k, v.clone());
        }

        query
    }

    #[inline]
    async fn do_query(&self, client: &Client, query: Vec<WriteQuery>) {
        // send query by chunk to avoid influxdb max request entity
        // error
        let chunks = query.chunks(self.batch_size);
        for ch in chunks {
            let batch = ch.to_owned();
            if let Err(e) = client.query(batch).await {
                panic!("Failed to write influxdb, {}", e)
            }
        }
    }

    fn report_meter(&self, name: &str, meter: &Meter, since_start: i64) -> WriteQuery {
        self.with_query(name, since_start)
            .add_field("m1", meter.m1_rate())
            .add_field("m5", meter.m5_rate())
            .add_field("m15", meter.m15_rate())
    }

    fn report_gauge(&self, name: &str, gauge: &Gauge, since_start: i64) -> WriteQuery {
        let value = gauge.value();
        self.with_query(name, since_start).add_field("value", value)
    }

    fn report_histogram(
        &self,
        name: &str,
        snapshot: &HistogramSnapshot,
        since_start: i64,
    ) -> WriteQuery {
        self.with_query(name, since_start)
            .add_field("p50", snapshot.quantile(0.5))
            .add_field("p75", snapshot.quantile(0.75))
            .add_field("p90", snapshot.quantile(0.90))
            .add_field("p99", snapshot.quantile(0.99))
            .add_field("p999", snapshot.quantile(0.999))
            .add_field("min", snapshot.min())
            .add_field("max", snapshot.max())
            .add_field("mean", snapshot.mean())
    }

    fn report_counter(&self, name: &str, c: &Counter, since_start: i64) -> WriteQuery {
        self.with_query(name, since_start)
            .add_field("value", c.value())
    }

    fn report_timer(&self, name: &str, t: &Timer, since_start: i64) -> WriteQuery {
        let rate = t.rate();
        let latency = t.latency();

        self.with_query(name, since_start)
            .add_field("p50", latency.quantile(0.5))
            .add_field("p75", latency.quantile(0.75))
            .add_field("p90", latency.quantile(0.90))
            .add_field("p99", latency.quantile(0.99))
            .add_field("p999", latency.quantile(0.999))
            .add_field("min", latency.min())
            .add_field("max", latency.max())
            .add_field("mean", latency.mean())
            .add_field("m1", rate.m1_rate())
            .add_field("m5", rate.m5_rate())
            .add_field("m15", rate.m15_rate())
    }

    fn system_time_millis(&self, since_start: i64) -> u128 {
        use std::ops::Add;
        let timestamp = self
            .test_start_time
            .add(std::time::Duration::from_nanos(
                since_start.try_into().unwrap(),
            ))
            .duration_since(UNIX_EPOCH);
        timestamp
            .expect("System time earlier than UNIX_EPOCH")
            .as_millis()
    }
}
