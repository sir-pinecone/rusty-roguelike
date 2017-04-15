#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterAttributes {
  pub max_hp: i32,
  pub hp: i32,
  pub defense: i32,
  pub power: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ai;


#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
  Heal
}
