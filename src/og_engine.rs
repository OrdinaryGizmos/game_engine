#![allow(non_snake_case)]
#![allow(deprecated)]
#![allow(clippy::too_many_arguments)]
//#![feature(nll)]

use crate::engine::OGEngine;

#[derive(Debug)]
pub enum Rcode {
    Fail,
    Ok,
    NoFile,
}

pub trait OGGame<D: 'static + OGData> {
    fn on_engine_start(&self, engine: &mut OGEngine<D>) -> Result<(), &str>;

    fn on_engine_update(&self, engine: &mut OGEngine<D>, elapsedTime: f64) -> Result<(), &str>;

    fn on_engine_destroy(&self, engine: &mut OGEngine<D>) -> Result<(), &str>;
}

pub trait OGData {}
