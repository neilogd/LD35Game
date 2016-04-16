extern crate sdl2;
extern crate time;

use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::Point;

pub mod math;

use math::*;
use std::f32::consts::{PI};
use time::*;

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
// Line
struct Line
{
	a: Vec2d,
	b: Vec2d,
}

/////////////////////////////////////////////////////////////////////
// Shape
struct Shape
{
	// Position for shape.
	position: Vec2d,
	// Points for shape.
	points: Vec<Vec2d>,
	// Function to update points given a 0.0-1.0 for where in shape, time, and morph.
	point_fn: Box<Fn(f32, f32, f32) -> Vec2d>,
	// Shape blend parameter.
	morph: f32,
}

impl Shape
{
	fn new(in_position: &Vec2d, num_points: usize, in_point_fn: Box<Fn(f32, f32, f32) -> Vec2d>) -> Shape 
	{
		let mut shape = Shape
		{
			position: Vec2d::new(in_position.x, in_position.y),
			points: Vec::with_capacity(num_points),
			point_fn: in_point_fn,
			morph: 0.0
		};
		shape.points.resize(num_points, Vec2d::new(0.0, 0.0));
		shape.update(0.0, 0.0);
		return shape;
	}

	fn update(&mut self, t: f32, morph: f32)
	{
		let num_points = self.points.len();
		let mul_val = 1.0 / num_points as f32;
		for idx in 0..num_points
		{
			self.points[idx] = (*self.point_fn)(idx as f32 * mul_val, t, morph);
		}
	}

	fn draw(&self, renderer: &mut sdl2::render::Renderer)
	{
		renderer.set_draw_color(Color::RGB(0, 192, 0));
		let num_points = self.points.len();
		for idx_a in 0..num_points
		{
			let idx_b = (idx_a + 1) % num_points;
			let point_a = self.points[idx_a] + self.position;
			let point_b = self.points[idx_b] + self.position;
			renderer.draw_line(point_a.get_point(), point_b.get_point());
		}
	}
}

fn get_time_seconds() -> f32
{
	precise_time_s() as f32
}

fn make_flat_wave_shape_function(size: f32, points: u32) -> Box<Fn(f32, f32, f32) -> Vec2d>
{
	let update_fn = move |x: f32, t: f32, b: f32| -> Vec2d
	{
		let rot = (x) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		return offset * size;
	};

	return Box::new(update_fn);
}

fn make_sine_wave_shape_function(size: f32, points: u32) -> Box<Fn(f32, f32, f32) -> Vec2d>
{
	let update_fn = move |x: f32, t: f32, b: f32| -> Vec2d
	{
		let rot = (x + t * 0.1) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		let scale_rot = (x * points as f32) * PI * 2.0;
		let scale = size * ((scale_rot.sin() + 1.0) / 2.0 + 1.0);
		return offset * scale;
	};

	return Box::new(update_fn);
}

fn make_triangle_wave_shape_function(size: f32, points: u32) -> Box<Fn(f32, f32, f32) -> Vec2d>
{
	let update_fn = move |x: f32, t: f32, b: f32| -> Vec2d
	{
		let triangle_fn = |x: f32|
		{
			let mod_x = (x * 2.0) % 2.0;
			if mod_x < 1.0
			{
				return mod_x;
			}
			return 1.0 - (mod_x - 1.0);
		};

		let rot = (x + t * 0.1) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		let scale_rot = x * points as f32;
		let scale = size * (triangle_fn(scale_rot) / 2.0 + 1.0);
		return offset * scale;
	};

	return Box::new(update_fn);
}

fn make_square_wave_shape_function(size: f32, points: u32) -> Box<Fn(f32, f32, f32) -> Vec2d>
{
	let update_fn = move |x: f32, t: f32, b: f32| -> Vec2d
	{
		let square_fn = |x: f32|
		{
			let mod_x = (x * 2.0) % 2.0;
			if mod_x < 1.0
			{
				return 0.0;
			}
			return 1.0;
		};

		let rot = (x + t * 0.1) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		let scale_rot = x * points as f32;
		let scale = size * (square_fn(scale_rot) / 2.0 + 1.0);
		return offset * scale;
	};

	return Box::new(update_fn);
}

fn make_combined_shape_function(size: f32, points: u32) -> Box<Fn(f32, f32, f32) -> Vec2d>
{
	let func_a = make_sine_wave_shape_function(size, points);
	let func_b = make_square_wave_shape_function(size, points);
	let update_fn = move |x: f32, t: f32, b: f32| -> Vec2d
	{
		return (func_a(x, t, b) * b) + (func_b(x, t, b) * (1.0 - b));
	};

	return Box::new(update_fn);	
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
	let mut tick = 0.0f32;
	let mut lastTime = get_time_seconds();

	let mut shapes = Vec::<Shape>::new();
	let mut shape_fns = Vec::<Box<Fn(f32, f32) -> Vec2d>>::new();

	let pos0 = Vec2d::new(1.0 * WIDTH as f32 / 4.0, 1.0 * HEIGHT as f32 / 4.0);
	let pos1 = Vec2d::new(3.0 * WIDTH as f32 / 4.0, 1.0 * HEIGHT as f32 / 4.0);
	let pos2 = Vec2d::new(1.0 * WIDTH as f32 / 4.0, 3.0 * HEIGHT as f32 / 4.0);
	let pos3 = Vec2d::new(3.0 * WIDTH as f32 / 4.0, 3.0 * HEIGHT as f32 / 4.0);

	shapes.push(Shape::new(&pos0, 64, make_combined_shape_function(96.0, 8)));
	shapes.push(Shape::new(&pos1, 64, make_sine_wave_shape_function(96.0, 8)));
	shapes.push(Shape::new(&pos2, 64, make_triangle_wave_shape_function(96.0, 8)));
	shapes.push(Shape::new(&pos3, 64, make_square_wave_shape_function(96.0, 8)));

	let mut blend = 0.0;

	'running: loop
	{
		for event in event_pump.poll_iter()
		{
			match event
			{
				Event::Quit {..} => break 'running,
				Event::KeyDown { keycode: Some(Keycode::Up), .. } => blend = blend + 0.1,
				Event::KeyDown { keycode: Some(Keycode::Down), .. } => blend = blend - 0.1,
				_ => {}
			}
		}

		// Clear screen.
		renderer.set_draw_color(Color::RGB(0, 0, 0));
		renderer.clear();
		
		for idx in 0..shapes.len()
		{
			let mut shape = &mut shapes[idx];
			shape.update(timer, blend);
		}

		for idx in 0..shapes.len()
		{
			let mut shape = &shapes[idx];
			shape.draw(&mut renderer);
		}

		renderer.present();

		// Timer handling.
		let nextTime = get_time_seconds();
		tick = nextTime - lastTime;
		lastTime = nextTime;
		timer = timer + tick;
	}
}

