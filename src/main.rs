use std::env;
use std::num::NonZeroU32;

use glutin::config::ConfigTemplateBuilder;
use raw_window_handle::HasRawWindowHandle;
use rusttype::Font; 
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event::{Event, MouseScrollDelta, ElementState};
use winit::event_loop::EventLoop;
use winit::window::CursorIcon;
use winit::window::WindowBuilder;
use winit::keyboard::{Key, NamedKey};
use glutin_winit::DisplayBuilder;
use winit::platform::macos::WindowBuilderExtMacOS;
use glutin::surface::WindowSurface;
use glutin::context::ContextAttributesBuilder;
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::SurfaceAttributesBuilder;

// use pager::render::terminal_render;
use pager::render::GlyphAtlas;
use pager::render::Display;
use pager::render::WindowConfig;
use pager::buffer::TextBuffer;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        log::error!("not enough args provided");
        std::process::exit(1);
    }
    let file_path = &env::args().collect::<Vec<_>>()[1];

    let font_data = include_bytes!("/Users/jason/Library/Fonts/Hack-Regular.ttf");
    let font = Font::try_from_bytes(font_data).expect("Error loading font");
    let font_size = 20.0;
    // let font_color = (0xab, 0xb2, 0xbf);
    let font_color = (0x00, 0x00, 0x00);
    let font_color = (font_color.0 as f32 / 255.0, font_color.1 as f32 / 255.0, font_color.2 as f32 / 255.0);
    let font_color = font_color;
    let atlas = GlyphAtlas::from_font(&font, font_size, font_color);

    if let Ok(buffer) = TextBuffer::from_filename(file_path) {
        // terminal_render(atlas.width, atlas.height, &atlas.buffer);
        run(atlas, font, font_size, buffer);
    } else {
        log::error!("file doesn't exist");
        std::process::exit(1);
    }
}

fn run(glyph_atlas: GlyphAtlas, font: Font<'static>, font_size: f32, mut buffer: TextBuffer) {
    let size = LogicalSize {width: 800, height: 600};

    let wb = WindowBuilder::new()
        .with_inner_size(size)
        .with_transparent(true)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_title_hidden(true);

    let event_loop = EventLoop::new().unwrap();
    let config_template_builder = ConfigTemplateBuilder::new();
    let display_builder = DisplayBuilder::new().with_window_builder(Some(wb));

    let (window, gl_config) = display_builder.build(&event_loop, config_template_builder, |mut configs| {
            // Just use the first configuration since we don't have any special preferences here
            configs.next().unwrap()
        })
        .unwrap();
    let window = window.unwrap();
    let raw_window_handle = window.raw_window_handle();
    let context_attributes = ContextAttributesBuilder::new().build(Some(raw_window_handle));

    let not_current_gl_context = Some(unsafe {
        gl_config.display().create_context(&gl_config, &context_attributes).unwrap()
    });

    // Determine our framebuffer size based on the window size, or default to 800x600 if it's invisible
    let (width, height): (u32, u32) = window.inner_size().into();
    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );
    // Now we can create our surface, use it to make our context current and finally create our display

    let surface = unsafe { gl_config.display().create_window_surface(&gl_config, &attrs).unwrap() };
    let context = not_current_gl_context.unwrap().make_current(&surface).unwrap();
    let display = glium::Display::new(context, surface).unwrap();

    let titlebar_height = 28.;
    let y_padding = 4.0 + titlebar_height;
    let x_padding = 10.0;
    let mut scroll_y = -y_padding; // we want to scroll beyond the top (ie. negative)

    let color = (0xFA, 0xFA, 0xFA);
    // let color = (0x28, 0x2c, 0x34);
    let bg_color = [color.0 as f32 / 255., color.1 as f32 / 255., color.2 as f32 / 255., 1.0];

    let window_things = WindowConfig::new(font_size, font, titlebar_height, x_padding, bg_color);

    let display = Display::new(glyph_atlas, display, window, window_things);

    event_loop.run(move |ev, elwt| {
        match ev {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    log::info!("close requested");
                    elwt.exit();
                },
                WindowEvent::Resized(_sized) => {
                    ()
                },
                WindowEvent::MouseWheel{delta, ..} => {
                    match delta {
                        MouseScrollDelta::LineDelta(_, y) => {
                            // Adjust the scroll position based on the scroll delta
                            scroll_y -= y * 20.0; // Adjust the scroll speed as needed
                            log::warn!("we don't expect a linedelta from mouse scroll on macOS, ignoring");
                        },
                        MouseScrollDelta::PixelDelta(PhysicalPosition{x: _, y}) => {
                            scroll_y -= y as f32;
                            // we want to scroll past the top (ie. negative)
                            let scale = rusttype::Scale::uniform(font_size);
                            let line_height = display.font().v_metrics(scale).ascent - display.font().v_metrics(scale).descent + display.font().v_metrics(scale).line_gap;
                            scroll_y = scroll_y.max(-y_padding).min((buffer.num_lines()-1) as f32 *line_height - titlebar_height);
                            match display.draw(&buffer, scroll_y, x_padding) {
                                Err(err) => log::error!("problem drawing: {:?}", err),
                                _ => ()
                            }
                        },
                    }
                },
                WindowEvent::ModifiersChanged(_state) => {
                    ()
                },
                WindowEvent::CursorMoved { device_id: _, position } => {
                    if position.y <= titlebar_height as f64 * 2. {
                        display.window.set_cursor_icon(CursorIcon::Default);
                    } else {
                        display.window.set_cursor_icon(CursorIcon::Text);
                    }
                },
                WindowEvent::KeyboardInput{device_id: _, event, is_synthetic: _} => {
                    let mut need_redraw = false;
                    if event.state != ElementState::Released {
                        match event.logical_key {
                            Key::Character(s) => {
                                buffer = buffer.insert(s.as_str());
                            },
                            Key::Named(n) => {
                                match n {
                                    NamedKey::Enter => buffer = buffer.insert("\n"),
                                    NamedKey::ArrowLeft => buffer = buffer.move_horizontal(-1),
                                    NamedKey::ArrowRight => buffer = buffer.move_horizontal(1),
                                    NamedKey::Space => buffer = buffer.insert(" "),
                                    NamedKey::Backspace => buffer = buffer.delete(),
                                    a => {dbg!(a);},
                                }
                            }
                            a => {dbg!(a);},
                        }
                        need_redraw = true;
                    }
                    if need_redraw {
                        match display.draw(&buffer, scroll_y, x_padding) {
                            Err(err) => log::error!("problem drawing: {:?}", err),
                            _ => ()
                        }
                    }
                },
                WindowEvent::RedrawRequested => {
                    log::info!("redraw requested");
                    match display.draw(&buffer, scroll_y, x_padding) {
                        Err(err) => log::error!("problem drawing: {:?}", err),
                        _ => ()
                    }
                },
                _ => (),
            },
            _ => (),
        }
    }).unwrap();
}
