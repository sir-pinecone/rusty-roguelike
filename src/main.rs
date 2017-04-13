extern crate tcod;
extern crate rand;

use std::env;
use std::cmp;
use rand::{Rng, SeedableRng, StdRng};
use tcod::console::*;
use tcod::colors::{self, Color};
use tcod::map::{Map as FovMap, FovAlgorithm};

mod components;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 30;

const MAP_WIDTH: i32 = SCREEN_WIDTH;
const MAP_HEIGHT: i32 = SCREEN_HEIGHT - 5;

const ROOM_MAX_SIZE: i32 = 12;
const ROOM_MIN_SIZE: i32 = 5;
// @feature min_rooms
const MAX_ROOMS: i32 = 10;
const MAX_ROOM_MONSTERS: i32 = 4;

const PLAYER_IDX: usize = 0; // Always the first object

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 100, b: 90 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color { r: 180, g: 160, b: 108 };

const DEFAULT_DEATH_CHAR: char = 'x';
const DEBUG_MODE: bool = true; // @incomplete make this a build flag

/* Mutably borrow two *separate elements from the given slice.
 * Panics when the indexes are equal or out of bounds.
 */
fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
  assert!(first_index != second_index);
  let split_at = cmp::max(first_index, second_index);
  let (first_splice, second_splice) = items.split_at_mut(split_at);
  if first_index < second_index {
    (&mut first_splice[first_index], &mut second_splice[0])
  } else {
    (&mut second_splice[0], &mut first_splice[second_index])
  }
}

struct ThreadContext {
  rand: StdRng,
  custom_seed: bool,
  rand_seed: i32
}

fn _new_thread_context_from_seed(seed_val: i32, rng_seed: &[usize], custom_seed: bool) -> ThreadContext {
    let rng: StdRng = SeedableRng::from_seed(rng_seed);
    println!("[RNG init] Provided seed: {:?}, RNG Seed: {:?}", seed_val, rng_seed[0]);
    ThreadContext {
      rand: rng,
      custom_seed: custom_seed,
      rand_seed: rng_seed[0] as i32
    }
}

impl ThreadContext {
  pub fn new() -> Self {
    let seed = 69; // default seed value
    let rng_seed: &[_] = &[&seed as *const i32 as usize];
    _new_thread_context_from_seed(seed, rng_seed, false)
  }

  pub fn from_seed(seed: i32) -> Self {
    let rng_seed: &[_] = &[seed as usize];
    _new_thread_context_from_seed(seed, rng_seed, true)
  }
}


#[derive(Debug)]
struct Object {
  x: i32,
  y: i32,
  char: char,
  death_char: char,
  color: Color,
  name: String,
  blocks: bool,
  alive: bool,
  show_when_dead: bool,

  // components
  char_attributes: Option<components::CharacterAttributes>,
  brain: Option<components::Ai>,
}

impl Object {
  pub fn new(x: i32, y: i32, char: char, death_char: char, name: &str, color: Color,
             blocks: bool, show_dead: bool) -> Self {
    Object {
      x: x,
      y: y,
      char: char,
      death_char: death_char,
      color: color,
      name: name.into(),
      blocks: blocks,
      alive: false,
      show_when_dead: show_dead,

      char_attributes: None,
      brain: None
    }
  }

  pub fn pos(&self) -> (i32, i32) {
    (self.x, self.y)
  }

  pub fn set_pos(&mut self, x: i32, y: i32) {
    self.x = x;
    self.y = y;
  }

  pub fn distance_to(&self, other: &Object) -> f32 {
    let dx = other.x - self.x;
    let dy = other.y - self.y;
    ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
  }

  // @incomplete switch to f32 for damage/health, etc
  pub fn take_damage(&mut self, damage: i32) {
    if damage > 0 {
      if let Some(char_attributes) = self.char_attributes.as_mut() {
        char_attributes.hp -= damage;
      }
    }
  }

  pub fn attack(&mut self, target: &mut Object) {
    let damage = self.char_attributes.map_or(0, |x| x.power) -
                 target.char_attributes.map_or(0, |x| x.defense);
    if damage > 0 {
      println!("{} attacks {} and deals {} damage!", self.name, target.name, damage);
      target.take_damage(damage);
    } else {
      println!("{} attacks {}, but it has no effect!", self.name, target.name);
    }
  }

  /* Draw the character that represents this object at its current position */
  pub fn draw(&self, con: &mut Console) {
    if self.alive || self.show_when_dead {
      let c = if self.alive {
        self.char
      } else {
        self.death_char
      };
      con.set_default_foreground(self.color);
      con.put_char(self.x, self.y, c, BackgroundFlag::None);
    }
  }

  /* Erase the character that represents this object */
  pub fn clear(&self, con: &mut Console) {
    con.put_char(self.x, self.y, ' ', BackgroundFlag::None);
  }
}


#[derive(Clone, Copy, Debug)]
struct Rect {
  x1: i32,
  x2: i32,
  y1: i32,
  y2: i32
}

impl Rect {
  pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
    Rect { x1: x, y1: y, x2: x + w, y2: y + h }
  }

  pub fn center(&self) -> (i32, i32) {
    let center_x = (self.x1 + self.x2) / 2;
    let center_y = (self.y1 + self.y2) / 2;
    (center_x, center_y)
  }

  pub fn intersects_with(&self, other: &Rect) -> bool {
    (self.x1 <= other.x2) && (self.x2 >= other.x1) &&
      (self.y1 <= other.y2) && (self.y2 >= other.y1)
  }
}


#[derive(Clone, Copy, Debug)]
struct Tile {
  // @future try using Object for tiles. Can then reuse HP, damage given, etc.
  passable: bool,
  blocks_sight: bool,
  explored: bool,
  visible: bool
}

impl Tile {
  pub fn empty() -> Self {
    Tile { passable: true, blocks_sight: false, explored: false, visible: false }
  }

  pub fn wall() -> Self {
    Tile { passable: false, blocks_sight: true, explored: false, visible: false }
  }

  pub fn make_empty(tile: &mut Tile) {
    tile.passable = true;
    tile.blocks_sight = false;
    tile.explored = false;
    tile.visible = false;
  }
}

type Map = Vec<Tile>;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
  TookTurn,
  DidntTakeTurn,
  Exit,
}

#[derive(Debug)]
struct TileCollisionInfo {
  collided: bool,
  collided_id: Option<usize>
}


/* Places a rect of empty tiles into `map` */
fn create_room(room: Rect, map: &mut Map) {
  for y in (room.y1 + 1)..room.y2 {
    for x in (room.x1 + 1)..room.x2 {
      map[(y * MAP_WIDTH + x) as usize] = Tile::empty();
    }
  }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
  for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
    Tile::make_empty(&mut map[(y * MAP_WIDTH + x) as usize]);
  }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
  for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
    Tile::make_empty(&mut map[(y * MAP_WIDTH + x) as usize]);
  }
}

fn make_map(thread_ctx: &mut ThreadContext, objects: &mut Vec<Object>) -> Map {
  let mut map = vec![Tile::wall(); (MAP_WIDTH * MAP_HEIGHT) as usize];
  let mut rooms = vec![];

  for i in 0..MAX_ROOMS {
    let w = thread_ctx.rand.gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
    let h = thread_ctx.rand.gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
    let x = thread_ctx.rand.gen_range(0, MAP_WIDTH - w);
    let y = thread_ctx.rand.gen_range(0, MAP_HEIGHT - h);

    let room = Rect::new(x, y, w, h);
    let can_place = !rooms.iter().any(|other_room| room.intersects_with(other_room));

    if can_place {
      create_room(room, &mut map);
      place_objects(thread_ctx, room, &map, objects);

      let (new_x, new_y) = room.center();

      if i == 0 {
        // @assumption we always create a room when i = 0 (first room created)
        objects[PLAYER_IDX].set_pos(new_x, new_y);
      } else {
        // connect to previous room with a tunnel
        let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

        // draw a coin to pick the type of tunnel
        if thread_ctx.rand.gen::<bool>() {
          create_h_tunnel(prev_x, new_x, prev_y, &mut map);
          create_v_tunnel(prev_y, new_y, new_x, &mut map);
        } else {
          create_v_tunnel(prev_y, new_y, prev_x, &mut map);
          create_h_tunnel(prev_x, new_x, new_y, &mut map);
        }
      }

      rooms.push(room);
    }
  }

  map
}

fn check_tile_for_object_collision(x: i32, y: i32, map: &Map,
                                   objects: &[Object]) -> TileCollisionInfo {
  let pos = (x, y);
  let id = objects.iter().position(|object| {
    object.pos() == pos
  });
  let collided = (id != None);
  let info = TileCollisionInfo { collided: collided, collided_id: id };
  return info;
}

fn is_tile_passable(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
  return map[(y * MAP_WIDTH + x) as usize].passable;
}

fn is_tile_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
  if !is_tile_passable(x, y, map, objects) {
    let tile_info = check_tile_for_object_collision(x, y, map, objects);
    return tile_info.collided;
  }
  return false;
}

fn place_objects(thread_ctx: &mut ThreadContext, room: Rect, map: &Map,
                 objects: &mut Vec<Object>) {
  let num_monsters = thread_ctx.rand.gen_range(0, MAX_ROOM_MONSTERS + 1);

  for _ in 0..num_monsters {
    // @incomplete if we can't place here then try again N times
    let x = thread_ctx.rand.gen_range(room.x1 + 1, room.x2);
    let y = thread_ctx.rand.gen_range(room.y1 + 1, room.y2);

    if !is_tile_blocked(x, y, map, objects) {
      let roll = thread_ctx.rand.next_f32();
      let mut monster = if roll < 0.4 {
        // Create a witch
        let mut witch = Object::new(x, y, 'W', DEFAULT_DEATH_CHAR, "Witch", colors::GREEN, true, true);
        witch.char_attributes = Some(components::CharacterAttributes {
          max_hp: 13, hp: 10, defense: 4, power: 3
        });
        witch.brain = Some(components::Ai);
        witch
      } else if roll < 0.7 {
        // Lizard
        let mut lizard = Object::new(x, y, 'L', DEFAULT_DEATH_CHAR, "Lizard", colors::DARKER_GREEN, true, true);
        lizard.char_attributes = Some(components::CharacterAttributes {
          max_hp: 7, hp: 5, defense: 2, power: 1
        });
        lizard.brain = Some(components::Ai);
        lizard
      } else {
        // Wizard
        let mut wizard = Object::new(x, y, '@', DEFAULT_DEATH_CHAR, "Evil Wizard", colors::RED, true, true);
        wizard.char_attributes = Some(components::CharacterAttributes {
          max_hp: 16, hp: 12, defense: 6, power: 4
        });
        wizard.brain = Some(components::Ai);
        wizard
      };

      monster.alive = true;
      objects.push(monster);
    }
  }
}

fn attempt_move(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object],
                on_collision: Option<&Fn(&mut Object)>) {
  let (x, y) = objects[id].pos();
  let new_x = x + dx;
  let new_y = y + dy;

  // @cleanup get collision info that includes a collision with a tile. Can use that to
  // change the properties of the tiles.
  if is_tile_passable(new_x, new_y, map, objects) {
    let info = check_tile_for_object_collision(new_x, new_y, map, objects);
    if info.collided {
      if on_collision.is_some() {
        let collision_id = info.collided_id.unwrap();
        on_collision.unwrap()(&mut objects[collision_id]);
      }
    } else {
      objects[id].set_pos(new_x, new_y);
    }
  }
}

fn move_towards(id: usize, (target_x, target_y): (i32, i32), map:&Map, objects: &mut [Object]) {
  let dx = target_x - objects[id].x;
  let dy = target_y - objects[id].y;
  let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

  let dx = (dx as f32 / distance).round() as i32;
  let dy = (dy as f32 / distance).round() as i32;
  attempt_move(id, dx, dy, map, objects, None);
}

fn player_attack(target: &mut Object) {
  println!("The {} laughs at your puny efforts to attack him!", target.name);
}

fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
  let on_collision = &player_attack;
  attempt_move(PLAYER_IDX, dx, dy, map, objects, Some(on_collision));
}

fn ai_take_turn(npc_id: usize, objects: &mut [Object], map: &Map, fov_map: &mut FovMap) {
  let (npc_x, npc_y) = objects[npc_id].pos();

  if fov_map.is_in_fov(npc_x, npc_y) {
    if objects[npc_id].distance_to(&objects[PLAYER_IDX]) >= 2.0 {
      let player_pos = objects[PLAYER_IDX].pos();
      move_towards(npc_id, player_pos, map, objects);
    }
    else if objects[PLAYER_IDX].alive {
      let (npc, player) = mut_two(npc_id, PLAYER_IDX, objects);
      npc.attack(player);
    }
  }
}

fn handle_input(root: &mut Root, map : &Map, objects: &mut [Object]) -> PlayerAction {
  use tcod::input::Key;
  use tcod::input::KeyCode::*;
  use PlayerAction::*;

  let key = root.wait_for_keypress(true);
  let is_player_alive = objects[PLAYER_IDX].alive;
  match (key, is_player_alive) {
    // Toggle fullscreen
    (Key { code: Enter, alt: true, .. }, _) => {
      let fullscreen = root.is_fullscreen();
      root.set_fullscreen(!fullscreen);
      DidntTakeTurn
    }

    // Exit game
    (Key { code: Escape, .. }, _) => Exit,

    // Movement
    (Key { code: Up, .. }, true) => {
      player_move_or_attack(0, -1, map, objects);
      TookTurn
    }
    (Key { code: Down, .. }, true) => {
      player_move_or_attack(0, 1, map, objects);
      TookTurn
    }
    (Key { code: Left, .. }, true) => {
      player_move_or_attack(-1, 0, map, objects);
      TookTurn
    }
    (Key { code: Right, .. }, true) => {
      player_move_or_attack(1, 0, map, objects);
      TookTurn
    }

    _ => DidntTakeTurn,
  }
}

fn update_map(map: &mut Map, fov_map: &mut FovMap, player_moved: bool) {
  // For now we only care about updating tile visibility and that only needs to happen
  // when the player moved
  if player_moved {
    for y in 0..MAP_HEIGHT {
      for x in 0..MAP_WIDTH {
        let tile = &mut map[(y * MAP_WIDTH + x) as usize];
        // @perf this can potentially be slow if we're dealing with a ton of tiles
        tile.visible = fov_map.is_in_fov(x, y);
        if tile.visible && !tile.explored {
          tile.explored = true;
        }
      }
    }
  }
}

// NOTE: We use the type &[Object] for objects because we want an immutable slice (a view)
fn render_all(root: &mut Root, con: &mut Offscreen, objects: &[Object],
              map: &Map, fov_map: &mut FovMap, render_map: bool) {
  // No need to re-render the map unless the FOV needs to be recomputed
  if render_map {
    for y in 0..MAP_HEIGHT {
      for x in 0..MAP_WIDTH {
        let tile = &map[(y * MAP_WIDTH + x) as usize];

        if tile.explored || tile.visible {
          let is_wall = tile.blocks_sight;
          let color = match(tile.visible, is_wall) {
            // Outside the FOV:
            (false, true) => COLOR_DARK_WALL,
            (false, false) => COLOR_DARK_GROUND,
            // Inside FOV:
            (true, true) => COLOR_LIGHT_WALL,
            (true, false) => COLOR_LIGHT_GROUND,
          };
          con.set_char_background(x, y, color, BackgroundFlag::Set);
        }
      }
    }
  }

  for object in objects {
    if fov_map.is_in_fov(object.x, object.y) {
      object.draw(con);
    }
  }

  blit(con, (0, 0), (MAP_WIDTH, MAP_HEIGHT), root, (0, 0), 1.0, 1.0);

  if let Some(char_attributes) = objects[PLAYER_IDX].char_attributes {
    root.print_ex(1, SCREEN_HEIGHT - 2, BackgroundFlag::None, TextAlignment::Left,
                  format!("HP: {}/{}, ALIVE: {}", char_attributes.hp, char_attributes.max_hp,
                          objects[PLAYER_IDX].alive));
  }
}

fn main() {
  let mut root = Root::initializer()
    .font("data/fonts/arial10x10.png", FontLayout::Tcod)
    .font_type(FontType::Greyscale)
    .size(SCREEN_WIDTH, SCREEN_HEIGHT)
    .title("Rusty Roguelike")
    .init();
  tcod::system::set_fps(LIMIT_FPS);

  let mut con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);

  // Setup the number generator
  let mut thread_ctx: ThreadContext;

  let mut provided_rng_seed: Option<i32> = None;
  let mut found_seed_flag = false;

  for argument in env::args() {
    if found_seed_flag {
      provided_rng_seed = Some(argument.trim().parse().expect("seed flag must be a number"));
    }
    if argument == "--seed" {
      found_seed_flag = true;
    }
  }

  match provided_rng_seed {
    Some(provided_rng_seed) => {
      thread_ctx = ThreadContext::from_seed(provided_rng_seed);
    }
    None => {
      thread_ctx = ThreadContext::new();
    }
  }

  let mut player = Object::new(0, 0, '@', 'X', "Player Bob", colors::WHITE, true, true);
  player.alive = true;
  player.char_attributes = Some(components::CharacterAttributes{
    max_hp: 30, hp: 30, defense: 2, power: 5
  });

  let mut objects = vec![player];

  let mut map = make_map(&mut thread_ctx, &mut objects);

  // Init fov
  let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
  for y in 0..MAP_HEIGHT {
    for x in 0..MAP_WIDTH {
      fov_map.set(x, y,
                  !map[(y * MAP_WIDTH + x) as usize].blocks_sight,
                  !map[(y * MAP_WIDTH + x) as usize].passable);
    }
  }

  let mut previous_player_pos = (-1, -1);
  let mut game_running = true;

  while game_running {
    let recompute_fov = previous_player_pos != (objects[PLAYER_IDX].x, objects[PLAYER_IDX].y);
    if recompute_fov {
      let player_ref = &objects[PLAYER_IDX];
      fov_map.compute_fov(player_ref.x, player_ref.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    update_map(&mut map, &mut fov_map, recompute_fov);
    render_all(&mut root, &mut con, &objects, &map, &mut fov_map, recompute_fov);

    if DEBUG_MODE {
      let mut seed_type_label = "Active";
      if thread_ctx.custom_seed {
        root.set_default_foreground(colors::RED);
        seed_type_label = "Custom";
      }
      else {
        root.set_default_foreground(colors::WHITE);
      }
      root.print_ex(1, SCREEN_HEIGHT - 4, BackgroundFlag::None, TextAlignment::Left,
                    format!("{} Seed Label: {}", seed_type_label, thread_ctx.rand_seed));
    }

    root.flush();

    // Erase objects at their old locations before moving
    for object in &objects {
      object.clear(&mut con);
    }

    previous_player_pos = objects[PLAYER_IDX].pos();

    // @idea allow the player to do things after death?
    // @idea copy the approach that Dwarf Fortress takes for world gen. Make a world and
    //   then persist it across lives. Allow people to drop out and play as a new character
    //   with the previous player being taken over by the game AI system.
    //   I particularly like the idea of leaving the corpse and allowing the next character
    //   to visit the body and take scraps if anything is still there.

    let player_action = handle_input(&mut root, &map, &mut objects);

    if player_action == PlayerAction::Exit || root.window_closed() {
      game_running = false;
    }

    // Update monsters
    if game_running && player_action == PlayerAction::TookTurn {
      for id in 0..objects.len() {
        if objects[id].brain.is_some() {
          ai_take_turn(id, &mut objects, &map, &mut fov_map);
        }
      }
    }
  }
}
