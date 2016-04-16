extern crate sdl2;

use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::Point;

pub mod math;

use math::Vec2d;

const WIDTH: i32 = 1024;
const HEIGHT: i32 = 768;


/////////////////////////////////////////////////////////////////////
// Utility
impl Vec2d
{
	fn get_point(&self) -> Point
	{
		Point::new(self.x as i32, self.y as i32)
	}
}


/////////////////////////////////////////////////////////////////////
// main
fn main()
{
	let ctx = sdl2::init().unwrap();
	let video_ctx = ctx.video().unwrap();

	let window = match video_ctx.window("LD35Game", WIDTH as u32, HEIGHT as u32).position_centered().opengl().build()
	{
		Ok(window) => window,
		Err(err) => panic!("Failed to create window: {}", err)
	};


	let mut renderer = window.renderer().build().unwrap();

	renderer.set_draw_color(Color::RGB(0, 0, 0));
	renderer.clear();
	renderer.present();

	let mut event_pump = ctx.event_pump().unwrap();

	let mut position = Vec2d::new(WIDTH as f32, HEIGHT as f32) * 0.5;

	let mut timer = 0.0f32;
	let tick = 1.0f32 / 60.0f32;

	'running: loop
	{
		for event in event_pump.poll_iter()
		{
			match event
			{
				Event::Quit {..} => break 'running,
				_ => {}
			}
		}

		// Clear screen.
		renderer.set_draw_color(Color::RGB(0, 0, 0));
		renderer.clear();

		let offset = Vec2d::new(timer.cos(), timer.sin());
		let point_a = position - (offset * 128.0);
		let point_b = position + (offset * 128.0);

		renderer.set_draw_color(Color::RGB(0, 192, 0));
		renderer.draw_line(point_a.get_point(), point_b.get_point());

		renderer.present();

		timer = timer + tick;
	}
}

