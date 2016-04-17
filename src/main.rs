extern crate sdl2;
extern crate time;

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::Point;
use sdl2::render::Renderer;

use sdl2::AudioSubsystem;

use std::sync::mpsc::{Sender, Receiver, channel};

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
// waves
fn sine_wave(x: f32) -> f32
{
	return (x * PI * 2.0).sin();
}

fn square_wave(x: f32) -> f32
{
	let mod_x = (x * 2.0) % 2.0;
	if mod_x < 1.0
	{
		return 0.0;
	}
	return 1.0;
}

fn triangle_wave(x: f32) -> f32
{
	let mod_x = (x * 2.0) % 2.0;
	if mod_x < 1.0
	{
		return mod_x;
	}
	return 1.0 - (mod_x - 1.0);
}

fn sawtooth_wave(x: f32) -> f32
{
	return x;
}


/////////////////////////////////////////////////////////////////////
// Line
struct Line
{
	a: Vec2d,
	b: Vec2d,
}

/////////////////////////////////////////////////////////////////////
// Mixer
type MixerFunc = Fn(f32) -> f32;

enum MixerChannel
{
	Sine(f32, f32),
	Square(f32, f32),
	Triangle(f32, f32),
	Sawtooth(f32, f32),
}

struct MixerChannelParams
{
	phase_inc: f32,
	phase: f32,
	volume: f32
}

impl MixerChannelParams
{
	fn default() -> MixerChannelParams
	{
		MixerChannelParams
		{
			 phase_inc: 0.0,
			 phase: 0.0,
			 volume: 0.0
		}
	}
}

struct MixerCallback
{
	freq: f32,
	rx: Receiver<MixerChannel>,
	channels: [MixerChannelParams; 4],
	channel_targets: [MixerChannelParams; 4],
}

impl AudioCallback for MixerCallback
{
	type Channel = f32;
	fn callback(&mut self, out: &mut [f32])
	{
		let result = self.rx.try_recv();

		match result
		{
			Ok(channel) => 
			{
				match channel
				{
					MixerChannel::Sine(f, v) =>
					{
						println!("Sine {}, {}", f, v);
						self.channel_targets[0].phase_inc = f / self.freq;
						self.channel_targets[0].volume = v;
					},
					MixerChannel::Square(f, v) =>
					{
						println!("Square {}, {}", f, v);
						self.channel_targets[1].phase_inc = f / self.freq;
						self.channel_targets[1].volume = v;
					},
					MixerChannel::Triangle(f, v) =>
					{
						println!("Triangle {}, {}", f, v);
						self.channel_targets[2].phase_inc = f / self.freq;
						self.channel_targets[2].volume = v;
					},
					MixerChannel::Sawtooth(f, v) =>
					{
						println!("Sawtooth {}, {}", f, v);
						self.channel_targets[3].volume = v;
						self.channel_targets[3].phase_inc = f / self.freq;
					},
				}
			}
			Err(_) => {}
		}

		for x in out.iter_mut()
		{
			let mut out_val = 0.0;
			for idx in 0..self.channels.len()
			{
				self.channels[idx].phase = (self.channels[idx].phase + self.channels[idx].phase_inc) % 1.0;

				let sample = match idx
				{
					0 => sine_wave( self.channels[idx].phase ),
					1 => square_wave( self.channels[idx].phase ),
					2 => triangle_wave( self.channels[idx].phase ),
					3 => sawtooth_wave( self.channels[idx].phase ),
					_ => 0.0,
				};
				out_val = out_val + self.channels[idx].volume * sample;				

				// Blend to target.
				self.channels[idx].phase_inc = self.channels[idx].phase_inc * 0.999 + self.channel_targets[idx].phase_inc * 0.001;
				self.channels[idx].volume = self.channels[idx].volume * 0.999 + self.channel_targets[idx].volume * 0.001;
			}

			*x = out_val;
		}
	}
}

struct Mixer
{
	// Audio device.
	device: AudioDevice<MixerCallback>
}

/////////////////////////////////////////////////////////////////////
// Shape
type PointFunc = Fn(f32, f32) -> Vec2d;
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
	// Sender for playing audio.
	audio_tx: Sender<MixerChannel>,
}

impl Shape
{
	fn new(
		in_audio_tx: &Sender<MixerChannel>, in_position: &Vec2d, num_points: usize, in_point_fn: Box<PointFunc>) -> Shape 
	{
		let mut shape = Shape
		{
			position: Vec2d::new(in_position.x, in_position.y),
			points: Vec::with_capacity(num_points),
			point_fns: [ Some(in_point_fn), Option::None ],
			curr_fn_idx: 0,
			morph: 0.0,
			audio_tx: in_audio_tx.clone(),
		};
		shape.points.resize(num_points, Vec2d::new(0.0, 0.0));
		shape.update(0.0, 0.0);
		return shape;
	}

	fn update(&mut self, tick: f32, time: f32)
	{
		let num_points = self.points.len();
		let mul_val = 1.0 / num_points as f32;
		{
			let curr_point_fn_opt = &self.point_fns[self.curr_fn_idx];
			let next_point_fn_opt = &self.point_fns[1 - self.curr_fn_idx];

			for idx in 0..num_points
			{
				let point_a : Vec2d;

				match curr_point_fn_opt.as_ref()
				{
					Some(curr_point_fn) => 
					{
						point_a = (*curr_point_fn)(idx as f32 * mul_val, time);
					},
					None => panic!("No curr_point_fn")
				}

				match next_point_fn_opt.as_ref()
				{
					Some(next_point_fn) => 
					{
						let point_b = (*next_point_fn)(idx as f32 * mul_val, time);
						self.points[idx] = (point_b * self.morph) + (point_a * (1.0 - self.morph));
					},
					None => self.points[idx] = point_a
				}
			}

			if next_point_fn_opt.is_some()
			{
				self.morph += tick;
			}
		}

		if self.morph > 1.0
		{
			self.point_fns[self.curr_fn_idx] = None;
			self.morph = 0.0;
			self.curr_fn_idx = 1 - self.curr_fn_idx;
		}
	}

	fn set_next(&mut self, point_fn: Box<PointFunc>)
	{
		self.point_fns[1 - self.curr_fn_idx] = Some(point_fn);
		self.morph = 0.0;
	}

	fn draw(&self, renderer: &mut Renderer)
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
	let update_fn = move |x: f32, t: f32| -> Vec2d
	{
		let rot = (x) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		return offset * size;
	};

	return Box::new(update_fn);
}

fn make_sine_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
{
	let update_fn = move |x: f32, t: f32| -> Vec2d
	{
		let rot = (x + t * 0.1) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		let scale_rot = x * points as f32;
		let scale = size * ((sine_wave(scale_rot) + 1.0) / 2.0 + 1.0);
		return offset * scale;
	};

	return Box::new(update_fn);
}

fn make_triangle_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
{
	let update_fn = move |x: f32, t: f32| -> Vec2d
	{
		let rot = (x + t * 0.1) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		let scale_rot = x * points as f32;
		let scale = size * (triangle_wave(scale_rot) / 2.0 + 1.0);
		return offset * scale;
	};

	return Box::new(update_fn);
}

fn make_square_wave_shape_function(size: f32, points: u32) -> Box<PointFunc>
{
	let update_fn = move |x: f32, t: f32| -> Vec2d
	{
		let rot = (x + t * 0.1) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());
		let scale_rot = x * points as f32;
		let scale = size * (square_wave(scale_rot) / 2.0 + 1.0);
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
	let audio_ctx = ctx.audio().unwrap();

	let window = match video_ctx.window("LD35Game", WIDTH as u32, HEIGHT as u32).position_centered().opengl().build()
	{
		Ok(window) => window,
		Err(err) => panic!("Failed to create window: {}", err)
	};

	let audio_spec = AudioSpecDesired
	{
		freq: Some(44100),
		channels: Some(1),
		samples: None
	};

	let (audio_tx, audio_rx) = channel();

	let audio = audio_ctx.open_playback(None, &audio_spec, |spec|
	{
		MixerCallback
		{
			freq: spec.freq as f32,
			rx: audio_rx,
			channels:
			[
				MixerChannelParams::default(),
				MixerChannelParams::default(),
				MixerChannelParams::default(),
				MixerChannelParams::default(),
			],
			channel_targets:
			[
				MixerChannelParams::default(),
				MixerChannelParams::default(),
				MixerChannelParams::default(),
				MixerChannelParams::default(),
			],

		}
	}).unwrap();
	audio.resume();


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


	shapes.push(Shape::new(&audio_tx, &pos0, 64, make_flat_wave_shape_function(96.0, 8)));
	shapes.push(Shape::new(&audio_tx, &pos1, 64, make_sine_wave_shape_function(96.0, 8)));
	shapes.push(Shape::new(&audio_tx, &pos2, 64, make_triangle_wave_shape_function(96.0, 8)));
	shapes.push(Shape::new(&audio_tx, &pos3, 64, make_square_wave_shape_function(96.0, 8)));

	audio_tx.send(MixerChannel::Sine(440.0, 0.1));
	audio_tx.send(MixerChannel::Square(440.0, 0.1));

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
			shape.update(tick, time);
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

