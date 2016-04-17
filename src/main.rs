extern crate sdl2;
extern crate time;
extern crate rand;

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Renderer, BlendMode};


pub mod math;

use std::io::prelude::*;
use std::fs::File;
use std::f32::consts::{PI};
use std::sync::mpsc::{Sender, Receiver, channel};
use math::*;
use time::*;
use rand::Rng;

const WIDTH: i32 = 1024;
const HEIGHT: i32 = 768;
const SIZE: f32 = 96.0;


/////////////////////////////////////////////////////////////////////
// Utility
impl Vec2d
{
	fn get_point(&self) -> Point
	{
		Point::new(self.x as i32, self.y as i32)
	}
}

fn draw_line(renderer: &mut Renderer, a: Vec2d, b: Vec2d)
{
	renderer.draw_line(a.get_point(), b.get_point());
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
	if mod_x > 1.0
	{
		return -1.0;
	}
	return 1.0;
}

fn sawtooth_wave(x: f32) -> f32
{
	return (x % 1.0) * 2.0 - 1.0;
}

fn envelope(x: f32, factor: f32) -> f32
{
	let mod_x = x % 1.0;
	return (factor * mod_x * (PI / (1.0 + (factor - 1.0) * mod_x))).sin();
}

/////////////////////////////////////////////////////////////////////
// Mixer
type MixerFunc = Fn(f32) -> f32;

#[derive(Copy, Clone, Debug)]
enum MixerChannel
{
	Sine(f32, f32),
	Square(f32, f32),
	Sawtooth(f32, f32),
	Beep(f32),
}

#[derive(Copy, Clone)]
struct MixerChannelParams
{
	phase_inc: f32,
	phase: f32,
	volume: f32,
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

	time: f32,
}

impl AudioCallback for MixerCallback
{
	type Channel = f32;
	fn callback(&mut self, out: &mut [f32])
	{
		'running: loop
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
						MixerChannel::Sawtooth(f, v) =>
						{
							self.channel_targets[2].volume = v;
							self.channel_targets[2].phase_inc = f / self.freq;
						},
						MixerChannel::Beep(f) =>
						{
							self.channel_targets[3].volume = 8.0;
							self.channel_targets[3].phase_inc = f / self.freq;
							self.channel_targets[3].phase = 0.0;
						}
					}
				}
				Err(_) => break 'running,
			}
		}

		for x in out.iter_mut()
		{
			let mut out_val = 0.0;
			for idx in 0..self.channels.len()
			{
				self.channels[idx].phase = (self.channels[idx].phase + self.channels[idx].phase_inc) % 1.0;

				let sample = match idx
				{
					0 => sine_wave( self.channels[idx].phase ) * envelope(self.time, 8.0),
					1 => square_wave( self.channels[idx].phase ) * envelope(self.time, 8.0),
					2 => sawtooth_wave( self.channels[idx].phase ) * envelope(self.time, 8.0),
					3 =>
					{
						self.channel_targets[idx].volume *= 0.995;
						sine_wave( self.channels[idx].phase )
					},
					_ => 0.0,
				};
				out_val = out_val + self.channels[idx].volume * sample;				

				// Blend to target.
				self.channels[idx].phase_inc = self.channels[idx].phase_inc * 0.999 + self.channel_targets[idx].phase_inc * 0.001;
				self.channels[idx].volume = self.channels[idx].volume * 0.999 + self.channel_targets[idx].volume * 0.001;
			}

			*x = out_val / 4.0;
			self.time = (self.time + 1.0 / self.freq) % 8.0;
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
	// Channels that have been set.
	set_channels: [MixerChannel; 3],
	// Channels for shape.
	channels: [MixerChannelParams; 3],
	// Channel targets for shape.
	channel_targets: [MixerChannelParams; 3],
	// Is selected shape?
	is_selected: bool,
}


impl Shape
{
	fn new(in_position: Vec2d, num_points: usize, in_channels: [MixerChannel; 3] ) -> Shape 
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
			],
			channel_targets:
			[
				MixerChannelParams::default(),
				MixerChannelParams::default(),
				MixerChannelParams::default(),
			],
			set_channels: in_channels.clone(),
			is_selected: false,
		};
		shape.points.resize(num_points, Vec2d::new(0.0, 0.0));
		shape.update(0.0, 0.0);
		shape.set_target(in_channels);
		return shape;
	}

	fn reset(&mut self, in_shape: &Shape)
	{
		self.position = in_shape.position;
		self.channel_targets = in_shape.channel_targets.clone();
		self.set_channels = in_shape.set_channels.clone();
		self.is_selected = false;
		self.update(0.0, 0.0);
	}

	fn set_target(&mut self, in_channels: [MixerChannel; 3])
	{
		let divisor = 440.0 / 8.0;
		for idx in 0..in_channels.len()
		{
			let (phase_inc, volume) = match in_channels[idx]
			{
				MixerChannel::Sine(f, v) => (f / divisor, v),
				MixerChannel::Square(f, v) => (f / divisor, v),
				MixerChannel::Sawtooth(f, v) => (f / divisor, v),
				_ => (0.0, 0.0),
			};

			self.channel_targets[idx].phase_inc = phase_inc;
			self.channel_targets[idx].volume = volume;
		}

		self.set_channels = in_channels;
	}

	fn play_audio(&mut self, audio_tx: &Sender<MixerChannel>)
	{
		for idx in 0..3
		{
			audio_tx.send(self.set_channels[idx]);
		}
		self.is_selected = true;
	}
	
	fn sample_channels(&self, x: f32, t: f32) -> Vec2d
	{
		let size = SIZE;
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
				2 => sawtooth_wave(scale_rot * channel.phase_inc) * channel.volume,
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

	fn draw(&self, renderer: &mut Renderer, color: Color)
	{
		renderer.set_draw_color(color);
		let num_points = self.points.len();
		for idx_a in 0..num_points
		{
			let idx_b = (idx_a + 1) % num_points;
			let point_a = self.points[idx_a] + self.position;
			let point_b = self.points[idx_b] + self.position;
			draw_line(renderer, point_a, point_b);
		}
	}
}

fn get_time_seconds() -> f32
{
	precise_time_s() as f32
}

fn build_shapes(level: u32) -> Vec<Shape>
{
	let mut shapes = Vec::<Shape>::new();
	let mut rng = rand::thread_rng();

	let pos = &mut [
		Vec2d::new(1.0 * WIDTH as f32 / 4.0, 2.0 * HEIGHT as f32 / 4.0),
		Vec2d::new(3.0 * WIDTH as f32 / 4.0, 2.0 * HEIGHT as f32 / 4.0),
		Vec2d::new(2.0 * WIDTH as f32 / 4.0, 2.0 * HEIGHT as f32 / 4.0),
	];

	if level >= 0
	{
		for shape_idx in 0..3
		{
			let channels = [
				MixerChannel::Sine(440.0, if shape_idx == 0 { 0.5 } else { 0.0 } ),
				MixerChannel::Square(440.0, if shape_idx == 1 { 0.5 } else { 0.0 } ),
				MixerChannel::Sawtooth(440.0, if shape_idx == 2 { 0.5 } else { 0.0 } ),
			];

			shapes.push(Shape::new(Vec2d::new(0.0, 0.0), 1024, channels));
		}
	}

	if level >= 5
	{
		for shape_idx in 0..3
		{
			let channels = [
				MixerChannel::Sine(880.0, if shape_idx == 0 { 0.5 } else { 0.0 } ),
				MixerChannel::Square(880.0, if shape_idx == 1 { 0.5 } else { 0.0 } ),
				MixerChannel::Sawtooth(880.0, if shape_idx == 2 { 0.5 } else { 0.0 } ),
			];

			shapes.push(Shape::new(Vec2d::new(0.0, 0.0), 1024, channels));
		}
	}

	if level >= 10
	{
		for shape_idx in 0..3
		{
			let channels = [
				MixerChannel::Sine(220.0, if shape_idx == 0 { 0.5 } else { 0.0 } ),
				MixerChannel::Square(220.0, if shape_idx == 1 { 0.5 } else { 0.0 } ),
				MixerChannel::Sawtooth(220.0, if shape_idx == 2 { 0.5 } else { 0.0 } ),
			];

			shapes.push(Shape::new(Vec2d::new(0.0, 0.0), 1024, channels));
		}
	}
	
	if level >= 20
	{
		for shape_idx in 0..3
		{
			let channels = [
				MixerChannel::Sine(110.0, if shape_idx == 0 { 0.5 } else { 0.0 } ),
				MixerChannel::Square(110.0, if shape_idx == 1 { 0.5 } else { 0.0 } ),
				MixerChannel::Sawtooth(110.0, if shape_idx == 2 { 0.5 } else { 0.0 } ),
			];

			shapes.push(Shape::new(Vec2d::new(0.0, 0.0), 1024, channels));
		}
	}
	

	if level >= 30
	{
		for shape_idx in 0..3
		{
			let channels = [
				MixerChannel::Sine(440.0, if shape_idx != 0 { 0.5 } else { 0.0 } ),
				MixerChannel::Square(440.0, if shape_idx != 1 { 0.5 } else { 0.0 } ),
				MixerChannel::Sawtooth(440.0, if shape_idx != 2 { 0.5 } else { 0.0 } ),
			];

			shapes.push(Shape::new(Vec2d::new(0.0, 0.0), 1024, channels));
		}
	}
		// Shuffle generated.
	for idx in 0 ..shapes.len()
	{
		let swap_idx = rng.gen::<usize>() % shapes.len();
		let val = shapes.swap_remove(swap_idx);
		shapes.push(val);
	}
	let mut new_shapes = Vec::<Shape>::new();


	if level < 20
	{
		new_shapes.push(shapes.swap_remove(0));
		new_shapes.push(shapes.swap_remove(0));

		new_shapes[0].position = pos[0];
		new_shapes[1].position = pos[1];
	}
	else 
	{
		new_shapes.push(shapes.swap_remove(0));
		new_shapes.push(shapes.swap_remove(0));
		new_shapes.push(shapes.swap_remove(0));

		new_shapes[0].position = pos[0];
		new_shapes[1].position = pos[1];
		new_shapes[2].position = pos[2];
	}

 	return new_shapes;
}

fn draw_char(renderer: &mut Renderer, position: Vec2d, scale: f32, color: Color, val: char)
{
	renderer.set_draw_color(color);
	let mut lines = Vec::<Vec2d>::new();
	match val
	{
		'0' =>
		{
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 2.0), Vec2d::new(0.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(0.0, 0.0)]);
		}
		'1' =>
		{
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 2.0)]);
		}
		'2' =>
		{
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 1.0), Vec2d::new(0.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(1.0, 2.0)]);
		}
		'3' =>
		{
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(1.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(1.0, 2.0)]);
		}
		'4' =>
		{
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 1.0), Vec2d::new(1.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(1.0, 2.0)]);
		}
		'5' => {
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(1.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(1.0, 2.0)]);
		}
		'6' => {
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 1.0), Vec2d::new(0.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(1.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(1.0, 2.0)]);
		}
		'7' => {
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 2.0)]);
		}
		'8' => {
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(0.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(1.0, 2.0)]);
		}
		'9' => {
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 0.0), Vec2d::new(1.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(0.0, 1.0)]);
			lines.append(&mut vec![Vec2d::new(1.0, 1.0), Vec2d::new(1.0, 2.0)]);
		}
		'X' => {
			lines.append(&mut vec![Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 2.0), Vec2d::new(1.0, 0.0)]);
		}
		'+' => {
			lines.append(&mut vec![Vec2d::new(0.5, 0.0), Vec2d::new(0.5, 2.0)]);
			lines.append(&mut vec![Vec2d::new(0.0, 1.0), Vec2d::new(1.0, 1.0)]);
		}
		'-' => {
			lines.append(&mut vec![Vec2d::new(0.0, 1.0), Vec2d::new(1.0, 1.0)]);
		}
		_ => {}
	}

	for idx in 0..(lines.len() / 2)
	{
		let point_a = lines[idx * 2] * scale;
		let point_b = lines[idx * 2 + 1] * scale;
		draw_line(renderer, position + point_a, position + point_b);
	}
}

fn draw_string(renderer: &mut Renderer, position: Vec2d, scale: f32, color: Color, vals: &String)
{
	let mut next_position = position;
	for val in vals.chars()
	{
		draw_char(renderer, next_position, scale, color, val);
		next_position = next_position + Vec2d::new(scale * 1.5, 0.0);
	}
}

/////////////////////////////////////////////////////////////////////
// Popup text
struct PopupText
{
	position: Vec2d,
	scale: f32,
	color: Color,
	time: f32,
	text: String,
}

impl PopupText
{
	fn new(in_position: Vec2d, in_scale: f32, in_color: Color, in_time: f32, in_text: String) -> PopupText
	{
		PopupText
		{
			position: in_position,
			scale: in_scale,
			color: in_color,
			time: in_time,
			text: in_text,
		}
	}

	fn draw(&mut self, renderer: &mut Renderer, tick: f32) -> bool
	{
		draw_string(renderer, self.position, self.scale, self.color, &self.text);
		self.position = self.position - Vec2d::new(0.0, self.scale * 4.0) * tick;
		self.time -= tick;
		return self.time > 0.0;
	}
}


/////////////////////////////////////////////////////////////////////
// main
fn main()
{
	let ctx = sdl2::init().unwrap();
	let video_ctx = ctx.video().unwrap();
	let audio_ctx = ctx.audio().unwrap();

	// Create window.
	let window = match video_ctx.window("LD35Game", WIDTH as u32, HEIGHT as u32).position_centered().opengl().build()
	{
		Ok(window) => window,
		Err(err) => panic!("Failed to create window: {}", err)
	};

	// Setup audio.
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
			time: 0.0,
		}
	}).unwrap();
	audio.resume();

	// Setup renderer.
	let mut renderer = window.renderer().build().unwrap();

	renderer.set_draw_color(Color::RGB(0, 0, 0));
	renderer.clear();
	renderer.present();

	let mut event_pump = ctx.event_pump().unwrap();

	let mut position = Vec2d::new(WIDTH as f32, HEIGHT as f32) * 0.5;

	let mut time = 0.0;
	let mut tick = 0.0;
	let mut last_time = get_time_seconds();

	let mut level = 1;
	let mut score = 0;
	let mut score_multiplier = 1;
	let mut high_score = 0;

	let high_score_filename = "highscore.txt";

	match File::open(high_score_filename)
	{
		Ok(file) =>
		{
			let mut opened_file = file;
			let mut s = String::new();
			opened_file.read_to_string(&mut s);
			high_score = match s.parse::<i32>()
			{
				Ok(v) => v,
				Err(..) => 0
			};
		}
		Err(e) => {}
	};


	let mut popup_texts = Vec::<PopupText>::new();

	let mut shapes = Vec::<Shape>::new();
	let mut mult = 1.0;
	let mut mouse_pos = Vec2d::new(0.0, 0.0);
	let mut rng = rand::thread_rng();
	let selected_shape_idx = rng.gen::<usize>() % 2;
	shapes = build_shapes(level);
	shapes[selected_shape_idx].play_audio(&audio_tx);

	'running: loop
	{
		for event in event_pump.poll_iter()
		{
			match event
			{
				Event::Quit {..} => break 'running,
				Event::MouseMotion { x, y, .. } => 
				{
					mouse_pos = Vec2d::new(x as f32, y as f32);
				},
				Event::MouseButtonDown { x, y, .. } =>
				{
					mouse_pos = Vec2d::new(x as f32, y as f32);
					let mut selected_idx = -1;
					for idx in 0..shapes.len()
					{
						let mut shape = &mut shapes[idx];
						if (mouse_pos - shape.position).magnitude() < SIZE
						{
							selected_idx = idx as i32;
						}
					}

					if selected_idx != -1
					{
						if shapes[selected_idx as usize].is_selected == true
						{
							let add_score = 10 * score_multiplier;
							score = score + add_score;
							level = level + 1;
							score_multiplier = score_multiplier + 1;

							popup_texts.push(PopupText::new(mouse_pos, 32.0, Color::RGB(0, 255, 0), 2.0, format!("+{}", add_score).to_string()));

							audio_tx.send(MixerChannel::Beep(1670.0));
						}
						else 
						{
							let sub_score = (score * 1) / 4;
							score = score - sub_score;
							level = (level * 3) / 4;
							if score < 0 
							{
								score = 0;
							}
							if level < 1
							{
								level = 1;
							}
							score_multiplier = 1;
							popup_texts.push(PopupText::new(mouse_pos, 32.0, Color::RGB(255, 0, 0), 2.0, format!("-{}", sub_score).to_string()));

							audio_tx.send(MixerChannel::Beep(110.0));
						}

						if score > high_score
						{
							high_score = score;
							let mut file = File::create(high_score_filename);
							file.unwrap().write_fmt(format_args!("{}", high_score));
						}

						let selected_shape_idx = rng.gen::<usize>() % 2;
						let new_shapes = build_shapes(level);

						if shapes.len() == new_shapes.len()
						{
							for idx in 0..shapes.len()
							{
								shapes[idx].reset(&new_shapes[idx]);
							}

						}
						else
						{
							shapes = new_shapes;
						}						
						shapes[selected_shape_idx].play_audio(&audio_tx);
					}
				},
				_ => {},
			}
		}

		// Update shapes.
		for idx in 0..shapes.len()
		{
			let mut shape = &mut shapes[idx];
			shape.update(tick, time);
		}

		// Clear screen.
		renderer.set_draw_color(Color::RGBA(0, 0, 0, 20));
		renderer.set_blend_mode(BlendMode::Blend);
		renderer.fill_rect(Rect::new(0, 0, WIDTH as u32, HEIGHT as u32));

		// Draw noise.
		{
			renderer.set_draw_color(Color::RGBA(0, 255, 0, 32));
			for idx in 0..4096
			{
				let x = rng.gen::<i32>() % WIDTH as i32;
				let y = rng.gen::<i32>() % HEIGHT as i32;

				renderer.draw_point(Point::new(x, y));
			}
		}

		// Draw shapes.
		for idx in 0..shapes.len()
		{
			let mut shape = &shapes[idx];

			let color = if (mouse_pos - shape.position).magnitude() < SIZE { Color::RGB(0, 255, 0) } else { Color::RGB(0, 128, 0) };

			shape.draw(&mut renderer, color);
		}

		// Draw score.
		draw_string(&mut renderer, Vec2d::new(128.0, 128.0 - 40.0), 16.0, Color::RGB(0, 128, 128), &high_score.to_string());
		draw_string(&mut renderer, Vec2d::new(128.0, 128.0), 16.0, Color::RGB(0, 128, 0), &score.to_string());

		// Draw popups.
		{
			let mut idx = 0 as usize;
			'popup: loop
			{
				if idx >= popup_texts.len()
				{
					break 'popup
				}
				if !popup_texts[idx].draw(&mut renderer, tick)
				{
					popup_texts.remove(idx);
				}
				else
				{
				    idx = idx + 1;
				}
			}
		}

		// Draw scanlines.
		{
			renderer.set_draw_color(Color::RGBA(0, 0, 0, 32));
			let mut y = 0.0;
			while y < HEIGHT as f32
			{

				draw_line(&mut renderer, Vec2d::new(0.0, y), Vec2d::new(WIDTH as f32, y as f32));

				y += 3.0;
			}
		}

		renderer.present();

		// Timer handling.
		let next_time = get_time_seconds();
		tick = next_time - last_time;
		last_time = next_time;
		time = time + tick;
	}
}
