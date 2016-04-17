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
	return x % 1.0;
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

#[derive(Copy, Clone)]
enum MixerChannel
{
	Sine(f32, f32),
	Square(f32, f32),
	Triangle(f32, f32),
	Sawtooth(f32, f32),
}

#[derive(Clone)]
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
						self.channel_targets[0].phase_inc = f / self.freq;
						self.channel_targets[0].volume = v;
					},
					MixerChannel::Square(f, v) =>
					{
						self.channel_targets[1].phase_inc = f / self.freq;
						self.channel_targets[1].volume = v;
					},
					MixerChannel::Triangle(f, v) =>
					{
						self.channel_targets[2].phase_inc = f / self.freq;
						self.channel_targets[2].volume = v;
					},
					MixerChannel::Sawtooth(f, v) =>
					{
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

			*x = out_val / 4.0;
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
	// Channels for shape.
	channels: [MixerChannelParams; 4],
	// Channel targets for shape.
	channel_targets: [MixerChannelParams; 4],
	// Sender for playing audio.
	audio_tx: Sender<MixerChannel>,
}

impl Shape
{
	fn new(
		in_audio_tx: &Sender<MixerChannel>, in_position: &Vec2d, num_points: usize, in_channels: [MixerChannel; 4] ) -> Shape 
	{
		let mut shape = Shape
		{
			position: Vec2d::new(in_position.x, in_position.y),
			points: Vec::with_capacity(num_points),
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
			audio_tx: in_audio_tx.clone(),
		};
		shape.points.resize(num_points, Vec2d::new(0.0, 0.0));
		shape.update(0.0, 0.0);
		shape.set_target(in_channels);
		return shape;
	}

	fn set_target(&mut self, in_channels: [MixerChannel; 4])
	{
		let divisor = 440.0 / 8.0;
		for idx in 0..in_channels.len()
		{
			let (phase_inc, volume) = match in_channels[idx]
			{
				MixerChannel::Sine(f, v) => (f / divisor, v),
				MixerChannel::Square(f, v) => (f / divisor, v),
				MixerChannel::Triangle(f, v) => (f / divisor, v),
				MixerChannel::Sawtooth(f, v) => (f / divisor, v),
			};

			self.channel_targets[idx].phase_inc = phase_inc;
			self.channel_targets[idx].volume = volume;
		}
		
	}

	fn sample_channels(&self, x: f32, t: f32) -> Vec2d
	{
		let size = 128.0;
		let rot = (x + t * 0.125) * PI * 2.0;
		let offset = Vec2d::new(rot.cos(), rot.sin());

		let mut out_sample = 0.0;

		let scale_rot = x;
		for idx in 0..self.channels.len()
		{
			let channel = &self.channels[idx];
			let sample = match idx
			{
				0 => sine_wave(scale_rot * channel.phase_inc) * channel.volume,
				1 => square_wave(scale_rot * channel.phase_inc) * channel.volume,
				2 => triangle_wave(scale_rot * channel.phase_inc) * channel.volume,
				3 => sawtooth_wave(scale_rot * channel.phase_inc) * channel.volume,
				_ => 0.0,
			};

			out_sample += sample;
		}
		
		let scale = size * (out_sample / 3.0 + 1.0);
		return offset * scale;
	}

	fn update(&mut self, tick: f32, time: f32)
	{
		for idx in 0..self.channels.len()
		{
			self.channels[idx].phase_inc = self.channels[idx].phase_inc * 0.95 + self.channel_targets[idx].phase_inc * 0.05;
			self.channels[idx].volume = self.channels[idx].volume * 0.95 + self.channel_targets[idx].volume * 0.05;
		}

		let num_points = self.points.len();
		let mul_val = 1.0 / num_points as f32;
		{
			for idx in 0..num_points
			{
				let point_a = self.sample_channels(idx as f32 * mul_val, time);
				self.points[idx] = point_a;
			}
		}
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

	let mut time = 0.0;
	let mut tick = 0.0;
	let mut lastTime = get_time_seconds();

	let mut shapes = Vec::<Shape>::new();

	let pos0 = Vec2d::new(1.0 * WIDTH as f32 / 4.0, 1.0 * HEIGHT as f32 / 4.0);
	let pos1 = Vec2d::new(3.0 * WIDTH as f32 / 4.0, 1.0 * HEIGHT as f32 / 4.0);
	let pos2 = Vec2d::new(1.0 * WIDTH as f32 / 4.0, 3.0 * HEIGHT as f32 / 4.0);
	let pos3 = Vec2d::new(3.0 * WIDTH as f32 / 4.0, 3.0 * HEIGHT as f32 / 4.0);

	let mut mult = 1.0;

	'running: loop
	{
		for event in event_pump.poll_iter()
		{
			match event
			{
				Event::Quit {..} => break 'running,
				Event::KeyDown { keycode: Some(Keycode::Up), .. } => mult = mult * 2.0,
				Event::KeyDown { keycode: Some(Keycode::Down), .. } => mult = mult / 2.0,
				Event::KeyDown { keycode: Some(Keycode::Left), .. } => 
				{
					let channels =
					[
						MixerChannel::Sine(440.0 * mult, 0.0),
						MixerChannel::Square(440.0 * mult, 0.0),
						MixerChannel::Triangle(440.0 * mult, 0.0),
						MixerChannel::Sawtooth(440.0 * mult, 1.0),
					];

					shapes.clear();
					shapes.push(Shape::new(&audio_tx, &pos0, 1024, channels.clone()));
					for idx in 0..channels.len()
					{
						audio_tx.send(channels[idx]);	
					}
				},
				Event::KeyDown { keycode: Some(Keycode::Right), .. } => 
				{

				},
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

