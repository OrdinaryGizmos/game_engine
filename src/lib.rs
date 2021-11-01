#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(deprecated)]
#![allow(unused)]
#![allow(clippy::too_many_arguments)]
#![feature(vec_into_raw_parts)]
//#![feature(nll)]

pub mod olc;
// pub mod app;
//pub mod audio;
pub mod camera;
pub mod collision;
// pub mod debug_gui;
pub mod decal;
pub mod engine;
pub mod game;
pub mod game_object;
pub mod geometry;
pub mod gltf_ext;
pub mod layer;
pub mod math_3d;
pub mod math_4d;
pub mod pixel;
pub mod platform;
pub mod renderer;
pub mod sprite;
//pub mod steam_audio;
pub mod texture;
pub mod transform;
pub mod util;

//pub mod steam_audio_bindgen;
use olc_pge_macros as macros;

pub mod prelude {
    pub use crate::{
        //audio, audio::*,
        camera, camera::*,
        collision, collision::*,
        decal, decal::*,
        engine, engine::*,
        game, game::*,
        game_object, game_object::*,
        geometry, geometry::*,
        gltf_ext, gltf_ext::*,
        layer, layer::*,
        math_3d, math_3d::*,
        math_4d, math_4d::*,
        pixel, pixel::*,
        platform, platform::*,
        renderer, renderer::*,
        sprite, sprite::*,
        texture, texture::*,
        transform, transform::*,
        util, util::*,
        //steam_audio_bindgen as phonon, steam_audio as effects,
        olc::Olc,
        olc::OlcData,
        olc::OlcFuture,
        olc::Rcode,
    };
}
