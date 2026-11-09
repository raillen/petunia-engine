//! Window and event-loop adapter.
//!
//! Opens a window with an OpenGL 3.3 core context (no Qt: the runtime never depends on the
//! editor toolkit), turns keyboard events into named actions, and drives the fly camera. The
//! window id of a key is its physical key code (`"KeyW"`, `"Space"`, ...), so bindings do not
//! depend on the keyboard layout.

// Creating a GL context and surface is `unsafe` in glutin; this adapter is where that is allowed.
#![allow(unsafe_code)]

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use glutin::config::{Config, ConfigTemplateBuilder};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, GlProfile, PossiblyCurrentContext, Version,
};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use petunia_input::{ActionMap, ActionState, actions};
use petunia_render::{
    FlyCamera, RenderFrame, SculptCache, TextureLoader, extract_frame, extract_sculpt_volumes,
    solid_shapes, start_camera, start_camera_at,
};
use petunia_render_gl::GlRenderer;
use petunia_script::{LoadReport, PhysicsSettings, PlayerInput, ScriptHost};
use petunia_world::World;
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{CursorGrabMode, Window, WindowId};

/// Window settings.
#[derive(Clone, Debug)]
pub struct PlayerConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// The project's `scripts/` folder. When set, the scripts run every frame.
    pub scripts: Option<std::path::PathBuf>,
    /// How the project is programmed (DR-40): `aipo`, `luau` or `nodes`. Only the files of that
    /// method load; a leftover file of the other one is ignored instead of running behind the
    /// person's back.
    pub programming_method: String,
    /// The project folder. When set, the sprites read their PNGs from it (`assets/hero.png`);
    /// without it a sprite is drawn as a plain colored quad.
    pub project_root: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerError {
    EventLoop(String),
    Window(String),
    OpenGl(String),
    Render(String),
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoop(message) => write!(
                formatter,
                "não foi possível iniciar o laço de eventos: {message}"
            ),
            Self::Window(message) => {
                write!(formatter, "não foi possível criar a janela: {message}")
            }
            Self::OpenGl(message) => write!(formatter, "OpenGL 3.3 indisponível: {message}"),
            Self::Render(message) => write!(formatter, "falha ao desenhar: {message}"),
        }
    }
}

impl std::error::Error for PlayerError {}

/// Opens the window and runs until it is closed. Blocks the calling thread.
pub fn run(world: World, config: PlayerConfig) -> Result<(), PlayerError> {
    let event_loop = EventLoop::new().map_err(|error| PlayerError::EventLoop(error.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(world, config);
    event_loop
        .run_app(&mut app)
        .map_err(|error| PlayerError::EventLoop(error.to_string()))?;
    match app.failure.take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

// Field order matters: the renderer must drop before the context and the window.
struct Graphics {
    renderer: GlRenderer,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    window: Window,
}

/// What is drawn and (when there are scripts) changed every frame.
enum Scene {
    Plain(World),
    Scripted(Box<ScriptHost>),
}

struct App {
    scene: Scene,
    config: PlayerConfig,
    fly: FlyCamera,
    sculpts: SculptCache,
    map: ActionMap,
    input: ActionState,
    graphics: Option<Graphics>,
    last_frame: Instant,
    cursor_grabbed: bool,
    failure: Option<PlayerError>,
    /// A character walks (W A S D, Space jumps) and the camera follows it, instead of flying.
    walking: bool,
    /// Pictures the scene names, decoded once from the project folder (DR-38).
    textures: TextureLoader,
    /// The game's own clock, in seconds: sprite sheets play from it (DR-39). The player always
    /// plays, so the clock only grows while the window is open.
    clock_seconds: f32,
}

impl App {
    // The player is the program the person sees: its console is the terminal it was started from.
    #[allow(clippy::print_stderr)]
    fn new(world: World, config: PlayerConfig) -> Self {
        let fly = petunia_project::player_start(&world)
            .map(|start| start_camera_at(start.eye, start.yaw_degrees))
            .unwrap_or_else(|| start_camera(&world));
        // Colliders, bodies or characters turn physics on, with or without scripts.
        let physical = ["Collider", "RigidBody", "Character"].iter().any(|table| {
            !world
                .entities_with_table(table)
                .unwrap_or_default()
                .is_empty()
        });
        let mut walking = false;
        let scene = if config.scripts.is_none() && !physical {
            Scene::Plain(world)
        } else {
            match ScriptHost::new(world.clone()) {
                Ok(mut host) => {
                    if let Some(folder) = config.scripts.as_deref() {
                        report_scripts(&host.load_directory(folder, &config.programming_method));
                    }
                    if physical {
                        // Someone to walk with, and everything drawn is solid.
                        if let Err(error) = petunia_project::ensure_player(&mut host.world_mut()) {
                            eprintln!("jogador não criado: {error}");
                        }
                        let solids = solid_shapes(&host.world());
                        match host.attach_physics(PhysicsSettings::default(), solids) {
                            Ok(problems) => {
                                for problem in problems {
                                    eprintln!("física: {problem}");
                                }
                            }
                            Err(message) => eprintln!("física desligada: {message}"),
                        }
                        walking = host.has_physics() && host.player_position().is_some();
                    }
                    Scene::Scripted(Box::new(host))
                }
                Err(message) => {
                    eprintln!("scripts e física desligados: {message}");
                    Scene::Plain(world)
                }
            }
        };
        Self {
            scene,
            config,
            fly,
            sculpts: SculptCache::new(),
            map: ActionMap::fly_defaults(),
            input: ActionState::new(),
            graphics: None,
            last_frame: Instant::now(),
            cursor_grabbed: false,
            failure: None,
            walking,
            textures: TextureLoader::new(),
            clock_seconds: 0.0,
        }
    }

    /// Decodes whatever the frame's sprites name (once per asset) and points them at it.
    fn load_textures(&mut self, frame: &mut RenderFrame) {
        let Some(root) = self.config.project_root.as_deref() else {
            return;
        };
        self.textures
            .resolve(frame, |asset| petunia_project::load_png_asset(root, asset));
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: PlayerError) {
        self.failure = Some(error);
        event_loop.exit();
    }

    fn set_cursor_grab(&mut self, grab: bool) {
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        self.cursor_grabbed = apply_cursor_grab(&graphics.window, grab);
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed_seconds = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        if self.input.pressed(&self.map, actions::RELEASE_CURSOR) {
            self.set_cursor_grab(false);
        }
        let right = self
            .input
            .axis(&self.map, actions::MOVE_LEFT, actions::MOVE_RIGHT);
        let up = self
            .input
            .axis(&self.map, actions::MOVE_DOWN, actions::MOVE_UP);
        let forward = self
            .input
            .axis(&self.map, actions::MOVE_BACKWARD, actions::MOVE_FORWARD);
        let boost = self.input.held(&self.map, actions::BOOST);
        let jump = self.input.pressed(&self.map, actions::MOVE_UP);
        if !self.walking {
            self.fly.advance(right, up, forward, boost, elapsed_seconds);
        }
        self.input.begin_frame();

        let mut frame = match &mut self.scene {
            Scene::Plain(world) => {
                let mut frame = extract_frame(world, self.fly.camera);
                frame.sculpts = self.sculpts.items(&extract_sculpt_volumes(world));
                frame
            }
            Scene::Scripted(host) => {
                if self.walking {
                    // The camera turns the opposite way around the vertical axis from a Transform.
                    host.set_player_input(PlayerInput {
                        forward,
                        right,
                        jump,
                        interact: false,
                        yaw_degrees: -self.fly.camera.yaw_degrees,
                    });
                }
                host.update(f64::from(elapsed_seconds));
                if self.walking
                    && let Some(position) =
                        host.third_person_camera(self.fly.camera.forward(), 4.5, 1.5)
                {
                    self.fly.camera.position = position;
                }
                report_run(host);
                let world = host.world();
                let mut frame = extract_frame(&world, self.fly.camera);
                frame.sculpts = self.sculpts.items(&extract_sculpt_volumes(&world));
                frame
            }
        };
        self.load_textures(&mut frame);
        // A game runs: every sprite sheet advances with the same clock the scripts and the physics
        // use (DR-39).
        self.clock_seconds += elapsed_seconds;
        frame.animate_sprites(self.clock_seconds);
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let size = graphics.window.inner_size();
        if let Err(error) = graphics.renderer.render(&frame, (size.width, size.height)) {
            self.fail(event_loop, PlayerError::Render(error.to_string()));
            return;
        }
        if let Err(error) = graphics.surface.swap_buffers(&graphics.context) {
            self.fail(event_loop, PlayerError::Render(error.to_string()));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        match create_graphics(event_loop, &self.config) {
            Ok(graphics) => {
                graphics.window.request_redraw();
                self.graphics = Some(graphics);
                self.last_frame = Instant::now();
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(graphics), Some(width), Some(height)) = (
                    self.graphics.as_ref(),
                    NonZeroU32::new(size.width),
                    NonZeroU32::new(size.height),
                ) {
                    graphics.surface.resize(&graphics.context, width, height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let key = format!("{code:?}");
                    match event.state {
                        ElementState::Pressed if !event.repeat => self.input.key_down(&key),
                        ElementState::Released => self.input.key_up(&key),
                        ElementState::Pressed => {}
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => self.set_cursor_grab(true),
            WindowEvent::Focused(false) => {
                self.input.release_all();
                self.set_cursor_grab(false);
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.cursor_grabbed
        {
            self.fly.look(delta.0 as f32, delta.1 as f32);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(graphics) = self.graphics.as_ref() {
            graphics.window.request_redraw();
        }
    }
}

fn apply_cursor_grab(window: &Window, grab: bool) -> bool {
    if grab {
        let grabbed = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .is_ok();
        window.set_cursor_visible(!grabbed);
        grabbed
    } else {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
        false
    }
}

/// glutin hands over every matching GL config and asks us to pick one. It always offers at
/// least one, and the picker has no way to return an error, so this is the single place where
/// the "never `expect`" rule has a recorded exception.
#[allow(clippy::expect_used)]
fn best_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .max_by_key(|config| config.num_samples())
        .expect("glutin offers at least one GL config")
}

fn create_graphics(
    event_loop: &ActiveEventLoop,
    config: &PlayerConfig,
) -> Result<Graphics, PlayerError> {
    let attributes = Window::default_attributes()
        .with_title(config.title.clone())
        .with_inner_size(LogicalSize::new(config.width, config.height));
    let template = ConfigTemplateBuilder::new().with_depth_size(24);
    let (window, gl_config) = DisplayBuilder::new()
        .with_window_attributes(Some(attributes.clone()))
        .build(event_loop, template, best_config)
        .map_err(|error| PlayerError::OpenGl(error.to_string()))?;

    let raw_handle = window
        .as_ref()
        .and_then(|window| window.window_handle().ok())
        .map(|handle| handle.as_raw());
    let display = gl_config.display();
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .with_profile(GlProfile::Core)
        .build(raw_handle);
    // SAFETY: the raw window handle belongs to a window that outlives the context.
    let not_current = unsafe { display.create_context(&gl_config, &context_attributes) }
        .map_err(|error| PlayerError::OpenGl(error.to_string()))?;

    let window = match window {
        Some(window) => window,
        None => glutin_winit::finalize_window(event_loop, attributes, &gl_config)
            .map_err(|error| PlayerError::Window(error.to_string()))?,
    };
    let surface_attributes = window
        .build_surface_attributes(Default::default())
        .map_err(|error| PlayerError::Window(error.to_string()))?;
    // SAFETY: the surface attributes come from the live window created above.
    let surface = unsafe { display.create_window_surface(&gl_config, &surface_attributes) }
        .map_err(|error| PlayerError::OpenGl(error.to_string()))?;
    let context = not_current
        .make_current(&surface)
        .map_err(|error| PlayerError::OpenGl(error.to_string()))?;
    // Vertical sync keeps the loop calm on modest hardware; ignore drivers that refuse it.
    let _ = surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN));

    // SAFETY: the context is current on this thread, so the loader resolves its functions.
    let gl =
        unsafe { glow::Context::from_loader_function_cstr(|name| display.get_proc_address(name)) };
    let renderer =
        GlRenderer::new(Arc::new(gl)).map_err(|error| PlayerError::OpenGl(error.to_string()))?;
    Ok(Graphics {
        renderer,
        surface,
        context,
        window,
    })
}

/// Scripts that did not load are said once, at the start.
// The player is the program the person sees: its console is the terminal it was started from.
#[allow(clippy::print_stderr)]
fn report_scripts(report: &LoadReport) {
    for (name, diagnostics) in &report.failed {
        for diagnostic in diagnostics.iter().filter(|d| d.is_error()) {
            eprintln!(
                "script `{name}` (linha {}): {}",
                diagnostic.line, diagnostic.message
            );
        }
    }
}

/// What scripts printed and the handlers that failed during the frame.
#[allow(clippy::print_stderr)]
fn report_run(host: &mut ScriptHost) {
    for line in host.take_log() {
        eprintln!("[{}] {}", line.level.name(), line.text);
    }
    host.take_faults();
}
