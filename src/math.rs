use std::ops::{ Add, Sub, Mul, Div };

/////////////////////////////////////////////////////////////////////
// Vec2d
#[derive(Copy, Clone)]
pub struct Vec2d
{
	pub x: f32,
	pub y: f32,
}

impl Vec2d
{
	pub fn new(in_x: f32, in_y: f32) -> Vec2d
	{
		Vec2d
		{
			x: in_x,
			y: in_y,
		}
	}

	pub fn magnitude(&self) -> f32
	{
		(self.x * self.x + self.y * self.y).sqrt()
	}
}

impl Add<Vec2d> for Vec2d
{
	type Output = Vec2d;
	fn add(self, rhs: Vec2d) -> Vec2d
	{
		Vec2d
		{
			x: self.x + rhs.x,
			y: self.y + rhs.y,
		}
	}
}

impl Add<f32> for Vec2d
{
	type Output = Vec2d;
	fn add(self, rhs: f32) -> Vec2d
	{
		Vec2d
		{
			x: self.x + rhs,
			y: self.y + rhs,
		}
	}
}

impl Sub<Vec2d> for Vec2d
{
	type Output = Vec2d;
	fn sub(self, rhs: Vec2d) -> Vec2d
	{
		Vec2d
		{
			x: self.x - rhs.x,
			y: self.y - rhs.y,
		}
	}
}

impl Sub<f32> for Vec2d
{
	type Output = Vec2d;
	fn sub(self, rhs: f32) -> Vec2d
	{
		Vec2d
		{
			x: self.x - rhs,
			y: self.y - rhs,
		}
	}
}

impl Mul<Vec2d> for Vec2d
{
	type Output = Vec2d;
	fn mul(self, rhs: Vec2d) -> Vec2d
	{
		Vec2d
		{
			x: self.x * rhs.x,
			y: self.y * rhs.y,
		}
	}
}

impl Mul<f32> for Vec2d
{
	type Output = Vec2d;
	fn mul(self, rhs: f32) -> Vec2d
	{
		Vec2d
		{
			x: self.x * rhs,
			y: self.y * rhs,
		}
	}
}

impl Div<Vec2d> for Vec2d
{
	type Output = Vec2d;
	fn div(self, rhs: Vec2d) -> Vec2d
	{
		Vec2d
		{
			x: self.x / rhs.x,
			y: self.y / rhs.y,
		}
	}
}

impl Div<f32> for Vec2d
{
	type Output = Vec2d;
	fn div(self, rhs: f32) -> Vec2d
	{
		Vec2d
		{
			x: self.x / rhs,
			y: self.y / rhs,
		}
	}
}
