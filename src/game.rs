//use crate::audio::AudioSystem;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::prelude::Rcode;

use super::{
    camera::Camera,
    decal::Decal,
    engine::OGEngine,
    layer::{LayerDesc, LayerFunc, LayerInfo, LayerType},
    og_engine::OGData,
    og_engine::OGGame,
    platform::{Platform, PlatformWindows, PLATFORM_DATA},
    renderer::Renderer,
    util::{HWButton, RoundTo, Vf2d, Vi2d},
};

use std::time::UNIX_EPOCH;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowBuilderExtWebSys;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    // The `console.log` is quite polymorphic, so we can bind it with multiple
    // signatures. Note that we need to use `js_name` to ensure we always call
    // `log` in JS.
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    // Multiple arguments too!
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}

pub fn construct<T: 'static + OGGame<D>, D: 'static + OGData>(
    game: T,
    game_data: D,
    app_name: &'static str,
    screen_width: u32,
    screen_height: u32,
    pixel_width: u32,
    pixel_height: u32,
    full_screen: bool,
    vsync: bool,
) {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Warn);

    unsafe {
        PLATFORM_DATA.init();
        PLATFORM_DATA.resolution = Some(Vi2d::from((
            (screen_width / pixel_width) as i32,
            (screen_height / pixel_height) as i32,
        )));
        if !full_screen {
            PLATFORM_DATA.window_size =
                Some(Vi2d::from((screen_width as i32, screen_height as i32)));
        }
        PLATFORM_DATA.full_screen = full_screen;
        PLATFORM_DATA.title = app_name.into();
        PLATFORM_DATA.pixel_size = Some(Vi2d::new(pixel_width as i32, pixel_height as i32));
    };
    let game_start = start_game(
        game,
        game_data,
        app_name,
        screen_width,
        screen_height,
        pixel_width,
        pixel_height,
        full_screen,
        vsync,
    );

    #[cfg(not(target_arch = "wasm32"))]
    futures::executor::block_on(game_start);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(game_start);
}

async fn finish_setup<D: 'static + OGData>(
    game_data: D,
    app_name: &'static str,
    screen_width: u32,
    screen_height: u32,
    pixel_width: u32,
    pixel_height: u32,
    full_screen: bool,
    vsync: bool,
    window: std::sync::Arc<winit::window::Window>,
    game: std::sync::Arc<dyn OGGame<D>>,
) -> OGEngine<D> {
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let body = document.body().unwrap();
        body.append_child(&canvas)
            .expect("Append canvas to HTML body");
    }

    let renderer: Renderer = Renderer::new(window.clone()).await;

    // let audio_system =
    //         AudioSystem::create_system();

    let mut engine = OGEngine {
        app_name: String::from(""),
        is_focused: true,
        window_width: 0,
        window_height: 0,
        pixels_w: 0,
        pixels_h: 0,
        pixel_width: 0,
        pixel_height: 0,
        inv_screen_size: Vf2d::new(0.0, 0.0),
        fps: 0,
        full_screen: false,
        renderer,
        game_data: Box::new(game_data),
        vsync: false,
        layers: vec![],
        draw_target: 0,
        mouse_position: Vi2d::from((0, 0)),
        font_decal: Decal::empty(),
        depth_buffer: vec![],
        camera: Camera::default(),
        //audio_system,
        window,
        game,
    };
    engine.init(
        app_name,
        screen_width,
        screen_height,
        pixel_width,
        pixel_height,
        full_screen,
        vsync,
    );
    engine
}

async fn start_game<T: OGGame<D> + 'static, D: OGData + 'static>(
    game: T,
    game_data: D,
    app_name: &'static str,
    screen_width: u32,
    screen_height: u32,
    pixel_width: u32,
    pixel_height: u32,
    full_screen: bool,
    vsync: bool,
) {
    let (window, event_loop) = PlatformWindows::create_window_pane(
        Vi2d { x: 10, y: 10 },
        unsafe { PLATFORM_DATA.window_size.unwrap() },
        unsafe { PLATFORM_DATA.full_screen },
    );

    let mut engine: OGEngine<D> = finish_setup(
        game_data,
        app_name,
        screen_width,
        screen_height,
        pixel_width,
        pixel_height,
        full_screen,
        vsync,
        window,
        std::sync::Arc::new(game),
    )
    .await;

    unsafe {
        if PLATFORM_DATA.full_screen {
            let fwin = PLATFORM_DATA.window_size.unwrap_or_default().to_vf2d();
            let fres = PLATFORM_DATA.resolution.unwrap_or_default().to_vf2d();
            PLATFORM_DATA.pixel_size = Some(
                (
                    (fwin.x as f32 / fres.x as f32) as i32,
                    (fwin.y as f32 / fres.y as f32) as i32,
                )
                    .into(),
            );
        }
    }
    engine.construct_font_sheet();
    engine.renderer.setup_layer_pipeline();
    engine.renderer.setup_3D_pipeline();
    //Create Primary Layer "0"
    let base_layer_id = engine.add_layer(LayerType::Image);
    let base_layer = engine.get_layer(base_layer_id).unwrap();
    engine.set_draw_target(base_layer_id);

    let mut frame_timer: f64 = 0.0;
    let mut frame_count: i32 = 0;
    let mut last_fps: i32 = 0;
    let mut frame_processed = true;
    let mut elapsed_time: f64 = 0.0;
    #[cfg(not(target_arch = "wasm32"))]
    let mut game_timer = UNIX_EPOCH.elapsed().unwrap().as_secs_f64();

    #[cfg(target_arch = "wasm32")]
    let mut game_timer = js_sys::Date::now() as f64;

    //game_engine.construct_font_sheet();
    let game = engine.game.clone();
    if let Err(message) = game.on_engine_start(&mut engine) {
        log::error!("{}", message);
        println!("{}", message);
        match event_loop
            .create_proxy()
            .send_event(Rcode::Fail)
        {
            Err(code) => log::error!("{code}"),
            Ok(code) => (),
        };
    }
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut engine);
    //(move |top_event, window_target|;
    //TODO: Setup physics engine on fixed time step
    //physics();

    engine.layers[0].shown = true;
}
