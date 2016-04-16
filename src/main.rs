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
type PointFunc = Fn(f32, f32, f32) -> Vec2d;
struct Shape
{
	// Position for shape.
	position: Vec2d,
	// Points for shape.
	points: Vec<Vec2d>,
	// Functions to update points given a 0.0-1.0 for where in shape, time, and morph.
	point_fns: [ Option<Box<PointFunc>>; 2 ],
	// Current function.
	curr_fn_idx: usize,
	// Shape blend parameter.
	morph: f32,
}

impl Shape
{
	fn new(in_position: &Vec2d, num_points: usize, in_point_fn: Box<PointFunc>) -> Shape 
	{
		let mut shape = Shape
		{
			position: Vec2d::new(in_position.x, in_position.y),
			points: Vec::with_capacity(num_points),
			point_fns: [ in_point_fn, Option::None ],
			curr_fn_idx: 0,
			morph: 0.0
		};
		shape.points.resize(num_points, Vec2d::new(0.0, 0.0));
		shape.update(0.0, 0.0, 0.0);
		return shape;
	}

	fn update(&mut self, tick: f32, time: f32, morph: f32)
	{
		let num_points = self.points.len();
		let mul_val = 1.0 / num_points as f32;
		{
			let &curr_point_fn = self.points_fns[self.curr_fn_idx];
			let &next_point_fn = self.points_fns[1 - self.curr_fn_idx];

			for idx in 0..num_points
			{

				let point_a = (*curr_point_fn)(idx as f32 * mul_val, time, morph);

				match next_point_fn.as_mut()
				{
					Some(next_point_fn) => 
					{
						let point_b = (*next_point_fn)(idx as f32 * mul_val, time, morph);
						self.points[idx] = (point_a * morph) + (point_b * (1.0 - morph));
						self.morph += tick;
					}
					None => self.points[idx] = point_a,
				}
			}
		}

		if self.morph > 1.0
		{
			let next_point_fn = self.next_point_fn.clone();
			//let next = *next_point_fn;
			//self.next_point_fn = None;
			//self.curr_point_fn = next_point_fn;
			self.morph = 0.0;
			self.curr_fn_idx = 1 - self.curr_fn_idx

		}
	}

	fn set_next(&mut self, point_fn: Box<PointFunc>)
	{
		self.next_point_fn = Some(point_fn);
		self.morph = 0.0;
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

fn make_flat_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
{
	let update_fn = move |x: f32, t: f32, b: f32| -> Vec2d
	{
		let rot = (x) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		return offset * size;
	};

	return Box::new(update_fn);
}

fn make_sine_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
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

fn make_triangle_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
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

fn make_square_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
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

	let mut time = 0.0f32;
	let mut tick = 0.0f32;
	let mut lastTime = get_time_seconds();

	let mut shapes = Vec::<Shape>::new();

	let pos0 = Vec2d::new(1.0 * WIDTH as f32 / 4.0, 1.0 * HEIGHT as f32 / 4.0);
	let pos1 = Vec2d::new(3.0 * WIDTH as f32 / 4.0, 1.0 * HEIGHT as f32 / 4.0);
	let pos2 = Vec2d::new(1.0 * WIDTH as f32 / 4.0, 3.0 * HEIGHT as f32 / 4.0);
	let pos3 = Vec2d::new(3.0 * WIDTH as f32 / 4.0, 3.0 * HEIGHT as f32 / 4.0);

	shapes.push(Shape::new(&pos0, 64, make_flat_wave_shape_function(96.0, 8)));
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
				Event::KeyDown { keycode: Some(Keycode::Left), .. } => shapes[0].set_next(make_sine_wave_shape_function(96.0, 8)),
				Event::KeyDown { keycode: Some(Keycode::Right), .. } => shapes[0].set_next(make_square_wave_shape_function(96.0, 8)),
				_ => {},
			}
		}

		// Clear screen.
		renderer.set_draw_color(Color::RGB(0, 0, 0));
		renderer.clear();
		
		for idx in 0..shapes.len()
		{
			let mut shape = &mut shapes[idx];
			shape.update(tick, time, blend);
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
		time = time + tick;
	}
}

