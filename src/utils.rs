use num_format::{Locale, ToFormattedString};
pub fn tostring<T: ToFormattedString>(arg: T) -> String {
    arg.to_formatted_string(&Locale::en)
}

pub fn tostringfloat(arg: f64) -> String {
    let f = arg.floor();
    let rest = arg - f;
    let rst = format!("{:.3}", rest);
    format!("{}{}", tostring(f as i64), rst[1..].to_string())
}

use metriki_core::global::global_registry;

pub struct Meter {
    sm: i64,
    co: i64,
    pub name: Option<String>,
}

impl Meter {
    pub fn new() -> Self {
        Meter {
            sm: 0,
            co: 0,
            name: None,
        }
    }

    pub fn inc(&mut self, d: i64) {
        self.co += 1;
        self.sm += d;
        self.name
            .as_ref()
            .map(|name| global_registry().meter(name.as_str()).mark());
    }

    pub fn stats(&self) -> String {
        if self.co == 0 {
            "0".to_string()
        } else {
            tostringfloat(self.sm as f64 / self.co as f64)
        }
    }
}

pub struct Counter {
    co: i64,
    pub name: Option<String>,
}

impl Counter {
    pub fn new() -> Self {
        Counter { co: 0, name: None }
    }

    pub fn inc(&mut self) {
        self.co += 1;
        self.name
            .as_ref()
            .map(|name| global_registry().counter(name.as_str()).inc(1));
    }

    pub fn stats(&self) -> String {
        tostring(self.co)
    }
}
