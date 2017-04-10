extern crate tcod;
extern crate rand;

use std::cmp;
use rand::{Rng, SeedableRng, StdRng};
use tcod::console::*;
use tcod::colors::{self, Color};
use tcod::map::{Map as FovMap, FovAlgorithm};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 30;

const MAP_WIDTH: i32 = SCREEN_WIDTH;
const MAP_HEIGHT: i32 = SCREEN_HEIGHT - 5;

const ROOM_MAX_SIZE: i32 = 12;
const ROOM_MIN_SIZE: i32 = 5;
// @feature min_rooms
const MAX_ROOMS: i32 = 10;
const MAX_ROOM_MONSTERS: i32 = 3;

const PLAYER_IDX: usize = 0; // Always the first object

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 100, b: 90 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color { r: 180, g: 160, b: 108 };


struct ThreadContext {
  rand: StdRng
}

impl ThreadContext {
  pub fn new(seed: &[usize]) -> Self {
    let rng: StdRng = SeedableRng::from_seed(seed);
    ThreadContext {
      rand: rng
    }
  }
}


#[derive(Debug)]
struct Object {
  x: i32,
  y: i32,
  char: char,
  color: Color,
}

impl Object {
  pub fn new(x: i32, y: i32, char: char, color: Color) -> Self {
    Object {
      x: x,
      y: y,
      char: char,
      color: color,
    }
  }

  pub fn set_pos(&mut self, x: i32, y: i32) {
    self.x = x;
    self.y = y;
  }

  /* Move by a given amount */
  pub fn move_by(&mut self, dx: i32, dy: i32, map: &Map) {
    let mut new_x = self.x + dx;
    let mut new_y = self.y + dy;

    if new_x >= MAP_WIDTH {
      new_x = 0;
    } else if new_x < 0 {
      new_x = MAP_WIDTH - 1;
    }

    if new_y >= MAP_HEIGHT {
      new_y = 0;
    } else if new_y < 0 {
      new_y = MAP_HEIGHT - 1;
    }

    if map[(new_y * MAP_WIDTH + new_x) as usize].passable {
      self.x = new_x;
      self.y = new_y;
    }
  }

  /* Draw the character that represents this object at its current position */
  pub fn draw(&self, con: &mut Console) {
    con.set_default_foreground(self.color);
    con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
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
  block_sight: bool,
  explored: bool,
  visible: bool
}

impl Tile {
  pub fn empty() -> Self {
    Tile { passable: true, block_sight: false, explored: false, visible: false }
  }

  pub fn wall() -> Self {
    Tile { passable: false, block_sight: true, explored: false, visible: false }
  }

  pub fn make_empty(tile: &mut Tile) {
    tile.passable = true;
    tile.block_sight = false;
    tile.explored = false;
    tile.visible = false;
  }
}

type Map = Vec<Tile>;

// Places a rect of empty tiles into `map`
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
      place_objects(thread_ctx, room, objects);

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

fn place_objects(thread_ctx: &mut ThreadContext, room: Rect, objects: &mut Vec<Object>) {
  let num_monsters = thread_ctx.rand.gen_range(0, MAX_ROOM_MONSTERS + 1);

  for _ in 0..num_monsters {
    let x = thread_ctx.rand.gen_range(room.x1 + 1, room.x2);
    let y = thread_ctx.rand.gen_range(room.y1 + 1, room.y2);

    let roll = thread_ctx.rand.next_f32();
    let monster = if roll < 0.4 {
      // Create a witch
      Object::new(x, y, 'W', colors::GREEN)
    } else if roll < 0.7 {
      // Lizard
      Object::new(x, y, 'L', colors::DARKER_GREEN)
    } else {
      // Wizard
      Object::new(x, y, '@', colors::RED)
    };

    objects.push(monster);
  }
}

fn handle_input(root: &mut Root, player: &mut Object, map : &Map) -> bool {
  use tcod::input::Key;
  use tcod::input::KeyCode::*;

  let key = root.wait_for_keypress(true);
  match key {
    // Toggle fullscreen
    Key { code: Enter, alt: true, .. } => {
      let fullscreen = root.is_fullscreen();
      root.set_fullscreen(!fullscreen);
    }

    // Exit game
    Key { code: Escape, .. } => return true,

    // Movement
    Key { code: Up, .. } => player.move_by(0, -1, map),
    Key { code: Down, .. } => player.move_by(0, 1, map),
    Key { code: Left, .. } => player.move_by(-1, 0, map),
    Key { code: Right, .. } => player.move_by(1, 0, map),

    _ => {}
  }
  false
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
          let is_wall = tile.block_sight;
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
  let seed_v = 69;
  let rng_seed: &[_] = &[&seed_v as *const i32 as usize];
  let mut thread_ctx = ThreadContext::new(&rng_seed);

  // @incomplete allow a seed to be fed to the program. Stick is in a seed var like so:
  // let rng_seed: &[_] = &[<value>];
  println!("Seed: {:?}", rng_seed);

  let player = Object::new(0, 0, '@', colors::WHITE);

  let mut objects = vec![player];

  let mut map = make_map(&mut thread_ctx, &mut objects);

  // Init fov
  let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
  for y in 0..MAP_HEIGHT {
    for x in 0..MAP_WIDTH {
      fov_map.set(x, y,
                  !map[(y * MAP_WIDTH + x) as usize].block_sight,
                  !map[(y * MAP_WIDTH + x) as usize].passable);
    }
  }

  let mut previous_player_pos = (-1, -1);

  while !root.window_closed() {
    let recompute_fov = previous_player_pos != (objects[PLAYER_IDX].x, objects[PLAYER_IDX].y);
    if recompute_fov {
      let player_ref = &objects[PLAYER_IDX];
      fov_map.compute_fov(player_ref.x, player_ref.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    update_map(&mut map, &mut fov_map, recompute_fov);
    render_all(&mut root, &mut con, &objects, &map, &mut fov_map, recompute_fov);

    root.flush();

    // Erase objects at their old locations before moving
    for object in &objects {
      object.clear(&mut con);
    }

    let player = &mut objects[PLAYER_IDX];
    previous_player_pos = (player.x, player.y);

    let exit = handle_input(&mut root, player, &map);
    if exit {
      break;
    }
  }
}
