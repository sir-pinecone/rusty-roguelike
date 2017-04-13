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


struct ThreadContext {
  rand: StdRng,
  provided_seed: i32,
  rand_seed: i32
}

fn _new_thread_context_from_seed(seed_val: i32, rng_seed: &[usize]) -> ThreadContext {
    let rng: StdRng = SeedableRng::from_seed(rng_seed);
    println!("[RNG init] Provided seed: {:?}, RNG Seed: {:?}", seed_val, rng_seed[0]);
    ThreadContext {
      rand: rng,
      provided_seed: seed_val,
      rand_seed: rng_seed[0] as i32
    }
}

impl ThreadContext {
  pub fn new() -> Self {
    let seed = 69; // default seed value
    let rng_seed: &[_] = &[&seed as *const i32 as usize];
    _new_thread_context_from_seed(seed, rng_seed)
  }

  pub fn from_seed(seed: i32) -> Self {
    let rng_seed: &[_] = &[seed as usize];
    _new_thread_context_from_seed(seed, rng_seed)
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
  show_when_dead: bool
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
      show_when_dead: show_dead
    }
  }

  pub fn pos(&self) -> (i32, i32) {
    (self.x, self.y)
  }

  pub fn set_pos(&mut self, x: i32, y: i32) {
    self.x = x;
    self.y = y;
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
        Object::new(x, y, 'W', DEFAULT_DEATH_CHAR, "Witch", colors::GREEN, true, true)
      } else if roll < 0.7 {
        // Lizard
        Object::new(x, y, 'L', DEFAULT_DEATH_CHAR, "Lizard", colors::DARKER_GREEN, true, true)
      } else {
        // Wizard
        Object::new(x, y, '@', DEFAULT_DEATH_CHAR, "Evil Wizard", colors::RED, true, true)
      };

      monster.alive = true;
      objects.push(monster);
    }
  }
}

fn attempt_move(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object],
                collided_with_object: &Fn(&mut Object)) {
  let (x, y) = objects[id].pos();
  let new_x = x + dx;
  let new_y = y + dy;

  if is_tile_passable(new_x, new_y, map, objects) {
    let info = check_tile_for_object_collision(new_x, new_y, map, objects);
    if info.collided {
      collided_with_object(&mut objects[info.collided_id.unwrap()]);
    } else {
      objects[id].set_pos(new_x, new_y);
    }
  }
}

fn player_attack(target: &mut Object) {
  println!("The {} laughs at your puny efforts to attack him!", target.name);
}

fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
  let collided_with_object = &player_attack;
  attempt_move(PLAYER_IDX, dx, dy, map, objects, collided_with_object);
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
  let mut thread_ctx = ThreadContext::new();
  //let mut thread_ctx = ThreadContext::from_seed(2811820);

  let mut player = Object::new(0, 0, '@', 'X', "Player Bob", colors::WHITE, true, true);
  player.alive = true;

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

  while !root.window_closed() {
    let recompute_fov = previous_player_pos != (objects[PLAYER_IDX].x, objects[PLAYER_IDX].y);
    if recompute_fov {
      let player_ref = &objects[PLAYER_IDX];
      fov_map.compute_fov(player_ref.x, player_ref.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    update_map(&mut map, &mut fov_map, recompute_fov);
    render_all(&mut root, &mut con, &objects, &map, &mut fov_map, recompute_fov);

    if DEBUG_MODE {
      // Render seed
      root.print_ex(1, SCREEN_HEIGHT - 5, BackgroundFlag::None, TextAlignment::Left,
                    format!("Provided seed: {}", thread_ctx.provided_seed));

      root.print_ex(1, SCREEN_HEIGHT - 4, BackgroundFlag::None, TextAlignment::Left,
                    format!("Active seed: {}", thread_ctx.rand_seed));
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
    if player_action == PlayerAction::Exit {
      break;
    }

    // Update monsters
    if objects[PLAYER_IDX].alive && player_action == PlayerAction::TookTurn {
      for id in 1..objects.len() {
        let object = &objects[id];
        // @incomplete only attack if next to player
        println!("The {} coughs at you!", object.name);
      }
    }
  }
}

