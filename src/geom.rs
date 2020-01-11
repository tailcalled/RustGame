use std::ops::*;

#[derive(Copy, Clone)]
pub struct Vec { x: i32, y:i32 }
#[derive(Copy, Clone)]
pub struct Dir(Vec);

impl Add for Vec {
	type Output = Vec;
	fn add(self, other: Self) -> Self {
		Vec { x : self.x + other.x, y : self.y + other.y }
	}
}
impl AddAssign for Vec {
	fn add_assign(&mut self, other: Self) {
		self.x += other.x;
		self.y += other.y;
	}
}
impl Sub for Vec {
	type Output = Vec;
	fn sub(self, other: Self) -> Self {
		Vec { x : self.x - other.x, y : self.y - other.y }
	}
}
impl SubAssign for Vec {
	fn sub_assign(&mut self, other: Self) {
		self.x -= other.x;
		self.y -= other.y;
	}
}

impl Dir {
	pub fn to_vec(self) -> Vec {
		let Dir(res) = self;
		res
	}
}