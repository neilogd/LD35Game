extern crate sdl2;
extern crate time;
extern crate rand;

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Renderer, BlendMode};

use sdl2::AudioSubsystem;

use std::sync::mpsc::{Sender, Receiver, channel};

pub mod math;

use math::*;
use std::f32::consts::{PI};
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

#[derive(Copy, Clone, Debug)]
enum MixerChannel
{
	Sine(f32, f32),
	Square(f32, f32),
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
	channels: [MixerChannelParams; 3],
	channel_targets: [MixerChannelParams; 3],
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
					0 => sine_wave( self.channels[idx].phase ),
					1 => square_wave( self.channels[idx].phase ),
					2 => sawtooth_wave( self.channels[idx].phase ),
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
	fn new(in_position: &Vec2d, num_points: usize, in_channels: [MixerChannel; 3] ) -> Shape 
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
			println!("{} {:?}", idx, self.set_channels[idx]);
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

fn build_shapes() -> Vec<Shape>
{
	let mut shapes = Vec::<Shape>::new();

	let pos = &mut [
		Vec2d::new(1.0 * WIDTH as f32 / 4.0, 2.0 * HEIGHT as f32 / 4.0),
		Vec2d::new(2.0 * WIDTH as f32 / 4.0, 2.0 * HEIGHT as f32 / 4.0),
		Vec2d::new(3.0 * WIDTH as f32 / 4.0, 2.0 * HEIGHT as f32 / 4.0),
	];

	for shape_idx in 0..3
	{
		let channels = [
			MixerChannel::Sine(440.0, if shape_idx == 0 { 0.5 } else { 0.0 } ),
			MixerChannel::Square(440.0, if shape_idx == 1 { 0.5 } else { 0.0 } ),
			MixerChannel::Sawtooth(440.0, if shape_idx == 2 { 0.5 } else { 0.0 } ),
		];

		shapes.push(Shape::new(&pos[shape_idx], 1024, channels));
	}

	return shapes;
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
			],
			channel_targets:
			[
				MixerChannelParams::default(),
				MixerChannelParams::default(),
				MixerChannelParams::default(),
			],
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

	let mut score = 0;
	let mut score_multiplier = 1;

	let mut popup_texts = Vec::<PopupText>::new();

	let mut shapes = Vec::<Shape>::new();
	let mut mult = 1.0;
	let mut mouse_pos = Vec2d::new(0.0, 0.0);
	let mut rng = rand::thread_rng();
	let selected_shape_idx = rng.gen::<usize>() % 3;
	shapes = build_shapes();
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
							score_multiplier = score_multiplier + 1;

							popup_texts.push(PopupText::new(mouse_pos, 32.0, Color::RGB(0, 255, 0), 2.0, format!("+{}", add_score).to_string()));
						}
						else 
						{
							let sub_score = score / 2;
							score = score - sub_score;
							if score < 0 
							{
								score = 0;
							}
							score_multiplier = 1;
							popup_texts.push(PopupText::new(mouse_pos, 32.0, Color::RGB(255, 0, 0), 2.0, format!("-{}", sub_score).to_string()));
						}

						let selected_shape_idx = rng.gen::<usize>() % 3;
						shapes = build_shapes();
						shapes[selected_shape_idx].play_audio(&audio_tx);
					}
				},
				_ => {},
			}
		}

		// Clear screen.
		renderer.set_draw_color(Color::RGBA(0, 0, 0, 16));
		renderer.set_blend_mode(BlendMode::Blend);
		renderer.fill_rect(Rect::new(0, 0, WIDTH as u32, HEIGHT as u32));
				
		for idx in 0..shapes.len()
		{
			let mut shape = &mut shapes[idx];
			shape.update(tick, time);
		}

		for idx in 0..shapes.len()
		{
			let mut shape = &shapes[idx];

			let color = if (mouse_pos - shape.position).magnitude() < SIZE { Color::RGB(0, 255, 0) } else { Color::RGB(0, 128, 0) };

			shape.draw(&mut renderer, color);
		}

		draw_string(&mut renderer, Vec2d::new(128.0, 128.0), 16.0, Color::RGB(0, 128, 0), &score.to_string());

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

		renderer.present();

		// Timer handling.
		let next_time = get_time_seconds();
		tick = next_time - last_time;
		last_time = next_time;
		time = time + tick;
	}
}
