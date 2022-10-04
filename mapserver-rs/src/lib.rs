pub mod coordinates;
pub mod mappool;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Extent(f64, f64, f64, f64);

impl Extent {
    pub fn from(e: (f64, f64, f64, f64)) -> Self {
        Extent(e.0, e.1, e.2, e.3)
    }
}
