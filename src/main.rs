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
const SCREEN_HEIGHT: i32 = 43;
const LIMIT_FPS: i32 = 30;

const MAP_WIDTH: i32 = SCREEN_WIDTH;
const MAP_HEIGHT: i32 = SCREEN_HEIGHT - 5;

const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

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

type Messages = Vec<(String, Color)>;

struct GameState {
  debug_mode: bool,
  debug_disable_fog: bool,
  messages: Messages,
  game_running: bool
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
  pub fn take_damage(&mut self, damage: i32, game_state: &mut GameState) {
    if self.alive && damage > 0 {
      if let Some(char_attributes) = self.char_attributes.as_mut() {
        char_attributes.hp -= cmp::min(damage, char_attributes.hp);
        if char_attributes.hp <= 0 {
          self.alive = false;
        }
      }
      if let Some(char_attributes) = self.char_attributes {
        if !self.alive {
          on_object_death(self, game_state);
        }
      }
    }
  }

  pub fn attack(&mut self, target: &mut Object, game_state: &mut GameState) {
    let damage = self.char_attributes.map_or(0, |x| x.power) -
                 target.char_attributes.map_or(0, |x| x.defense);
    if damage > 0 {
      message(&mut game_state.messages, format!("{} attacks {} and deals {} damage!", self.name, target.name, damage), colors::WHITE);
      target.take_damage(damage, game_state);
    } else {
      message(&mut game_state.messages, format!("{} attacks {}, but it has no effect!", self.name, target.name), colors::WHITE);
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

  // @feature show a list of objects that reside on a tile
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
  collision: bool,
  obj_collision: bool,
  tile_collision: bool,
  collision_id: Option<usize>
}

fn message<T: Into<String>>(messages: &mut Messages, message: T, color: Color) {
  if messages.len() == MSG_HEIGHT {
    messages.remove(0);
  }
  messages.push((message.into(), color));
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

      let (new_x, new_y) = room.center();

      if rooms.is_empty() {
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

      place_objects(thread_ctx, room, &map, objects);

      rooms.push(room);
    }
  }

  map
}

fn check_tile_for_collision(x: i32, y: i32, map: &Map, objects: &[Object]) -> TileCollisionInfo {
  let mut coll_info = TileCollisionInfo {
    collision: false,
    obj_collision: false,
    tile_collision: false,
    collision_id: None
  };

  let tile_passable = map[(y * MAP_WIDTH + x) as usize].passable;
  if tile_passable {
    // Find object collision
    let pos = (x, y);
    let id = objects.iter().position(|object| {
      object.blocks && (object.pos() == pos)
    });
    let collision = (id != None);

    coll_info.collision = collision;
    coll_info.obj_collision = collision;
    coll_info.collision_id = id;
  }
  else {
    coll_info.collision = true;
    coll_info.tile_collision = true;
  }
  return coll_info;
}

fn npc_name(label: &str, objects: &[Object]) -> String {
  let s = format!("{}_{:}", label, objects.len() + 1);
  return s;
}

fn place_objects(thread_ctx: &mut ThreadContext, room: Rect, map: &Map,
                 objects: &mut Vec<Object>) {
  let num_monsters = thread_ctx.rand.gen_range(0, MAX_ROOM_MONSTERS + 1);

  for _ in 0..num_monsters {
    // @incomplete if we can't place here then try again N times
    let x = thread_ctx.rand.gen_range(room.x1 + 1, room.x2);
    let y = thread_ctx.rand.gen_range(room.y1 + 1, room.y2);

    let coll_info = check_tile_for_collision(x, y, map, objects);
    if !coll_info.collision {
      let roll = thread_ctx.rand.next_f32();
      let mut monster = if roll < 0.4 {
        // Create a witch
        let name = npc_name("Witch", objects);
        let mut witch = Object::new(x, y, 'W', DEFAULT_DEATH_CHAR, &name, colors::GREEN, true, true);
        witch.char_attributes = Some(components::CharacterAttributes {
          max_hp: 13, hp: 10, defense: 4, power: 3
        });
        witch.brain = Some(components::Ai);
        witch
      } else if roll < 0.7 {
        // Lizard
        let name = npc_name("Lizard", objects);
        let mut lizard = Object::new(x, y, 'L', DEFAULT_DEATH_CHAR, &name, colors::DARKER_GREEN, true, true);
        lizard.char_attributes = Some(components::CharacterAttributes {
          max_hp: 7, hp: 5, defense: 2, power: 1
        });
        lizard.brain = Some(components::Ai);
        lizard
      } else {
        // Wizard
        let name = npc_name("Wizard", objects);
        let mut wizard = Object::new(x, y, '@', DEFAULT_DEATH_CHAR, &name, colors::RED, true, true);
        wizard.char_attributes = Some(components::CharacterAttributes {
          max_hp: 16, hp: 12, defense: 3, power: 4
        });
        wizard.brain = Some(components::Ai);
        wizard
      };

      monster.alive = true;
      objects.push(monster);
    }
  }
}

fn on_object_death(obj: &mut Object, game_state: &mut GameState) {
  match obj.brain {
    Some(brain) => {
      // AI
      message(&mut game_state.messages, format!("{} died!", obj.name), colors::RED);
      obj.blocks = false;
      obj.brain = None
    },
    // player
    None => {
      message(&mut game_state.messages, format!("Player {} died!", obj.name), colors::RED);
      obj.blocks = false;
    }
  }
}

fn attempt_move(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) -> TileCollisionInfo {
  let (x, y) = objects[id].pos();
  let new_x = x + dx;
  let new_y = y + dy;

  let coll_info = check_tile_for_collision(new_x, new_y, map, objects);
  if !coll_info.collision {
    objects[id].set_pos(new_x, new_y);
  }
  return coll_info;
}

fn move_towards(id: usize, (target_x, target_y): (i32, i32), map:&Map, objects: &mut [Object]) {
  let dx = target_x - objects[id].x;
  let dy = target_y - objects[id].y;
  let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

  let dx = (dx as f32 / distance).round() as i32;
  let dy = (dy as f32 / distance).round() as i32;
  attempt_move(id, dx, dy, map, objects);
}

fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut [Object],
                         game_state: &mut GameState) {
  let coll_info = attempt_move(PLAYER_IDX, dx, dy, map, objects);
  if coll_info.obj_collision && coll_info.collision_id.is_some() {
    let (player, target) = mut_two(PLAYER_IDX, coll_info.collision_id.unwrap(), objects);
    if target.alive {
      player.attack(target, game_state);
    }
    else {
      message(&mut game_state.messages, format!("{} chops at the corpse of {}. Blood sprays out.", player.name, target.name), colors::BLUE);
    }
  }
}

fn ai_take_turn(npc_id: usize, objects: &mut [Object], map: &Map, fov_map: &mut FovMap,
                game_state: &mut GameState) {
  let (npc_x, npc_y) = objects[npc_id].pos();

  if fov_map.is_in_fov(npc_x, npc_y) {
    if objects[npc_id].distance_to(&objects[PLAYER_IDX]) >= 2.0 {
      let player_pos = objects[PLAYER_IDX].pos();
      move_towards(npc_id, player_pos, map, objects);
    }
    else if objects[PLAYER_IDX].alive {
      let (npc, player) = mut_two(npc_id, PLAYER_IDX, objects);
      npc.attack(player, game_state);
    }
  }
}

fn handle_input(root: &mut Root, map : &Map, objects: &mut [Object],
                game_state: &mut GameState) -> PlayerAction {
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
      player_move_or_attack(0, -1, map, objects, game_state);
      TookTurn
    }
    (Key { code: Down, .. }, true) => {
      player_move_or_attack(0, 1, map, objects, game_state);
      TookTurn
    }
    (Key { code: Left, .. }, true) => {
      player_move_or_attack(-1, 0, map, objects, game_state);
      TookTurn
    }
    (Key { code: Right, .. }, true) => {
      player_move_or_attack(1, 0, map, objects, game_state);
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

fn render_bar(panel: &mut Offscreen, x: i32, y: i32, total_width: i32, name: &str,
              value: i32, maximum: i32, bar_color: Color, back_color: Color) {
  let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

  panel.set_default_background(back_color);
  panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

  panel.set_default_background(bar_color);
  if bar_width > 0 {
    panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
  }

  panel.set_default_background(colors::WHITE);
  panel.print_ex(x + total_width / 2, y, BackgroundFlag::None, TextAlignment::Center,
                 &format!("{}: {}/{}", name, value, maximum));
}

// NOTE: We use the type &[Object] for objects because we want an immutable slice (a view)
fn render_all(game_state: &GameState, root: &mut Root, con: &mut Offscreen,
              panel: &mut Offscreen, objects: &[Object], map: &Map, fov_map: &mut FovMap,
              render_map: bool) {
  // No need to re-render the map unless the FOV needs to be recomputed
  if render_map {
    for y in 0..MAP_HEIGHT {
      for x in 0..MAP_WIDTH {
        let tile = &map[(y * MAP_WIDTH + x) as usize];

        if tile.explored || game_state.debug_disable_fog || tile.visible {
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

  let mut to_draw: Vec<_> = objects.iter()
                                    .filter(|o| game_state.debug_disable_fog || fov_map.is_in_fov(o.x, o.y))
                                    .collect();

  to_draw.sort_by(|o1, o2| { o1.blocks.cmp(&o2.blocks) });
  for obj in &to_draw {
    obj.draw(con);
  }

  blit(con, (0, 0), (MAP_WIDTH, MAP_HEIGHT), root, (0, 0), 1.0, 1.0);

  // Show stats
  panel.set_default_background(colors::BLACK);
  panel.clear();

  let hp = objects[PLAYER_IDX].char_attributes.map_or(0, |f| f.hp);
  let max_hp = objects[PLAYER_IDX].char_attributes.map_or(0, |f| f.max_hp);
  render_bar(panel, 1, 1, BAR_WIDTH, "HP", hp, max_hp, colors::LIGHT_RED, colors::DARKER_RED);

  // Game messages
  let mut y = MSG_HEIGHT as i32;
  for &(ref msg, color) in game_state.messages.iter().rev() {
    let msg_height = panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
    y -= msg_height;
    if y < 0 {
      break;
    }
    panel.set_default_foreground(color);
    panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
  }

  blit(panel, (0, 0), (SCREEN_WIDTH, PANEL_HEIGHT), root, (0, PANEL_Y), 1.0, 1.0);
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
  let mut panel = Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT);

  let mut game_state = GameState {
    debug_mode: false,
    debug_disable_fog: false,
    messages: vec![],
    game_running: true,
  };

  // Setup the number generator
  let mut thread_ctx: ThreadContext;

  let mut provided_rng_seed: Option<i32> = None;
  let mut found_seed_flag = false;
  let mut found_debug_flag = false;

  for argument in env::args() {
    if found_seed_flag {
      provided_rng_seed = Some(argument.trim().parse().expect("seed flag must be a number"));
      found_seed_flag = false;
    } else if found_debug_flag {
      game_state.debug_mode = (argument.trim() != "false");
      found_debug_flag = false;
    }
    else {
      match argument.as_ref() {
        "--seed"        => found_seed_flag = true,
        "--debug"       => found_debug_flag = true,
        "--disable-fog" => game_state.debug_disable_fog = true,
        _ => {}
      };
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
    max_hp: 30, hp: 30, defense: 3, power: 7
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

  while game_state.game_running {
    let recompute_fov = previous_player_pos != (objects[PLAYER_IDX].x, objects[PLAYER_IDX].y);
    if recompute_fov {
      let player_ref = &objects[PLAYER_IDX];
      fov_map.compute_fov(player_ref.x, player_ref.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    update_map(&mut map, &mut fov_map, recompute_fov);
    render_all(&game_state, &mut root, &mut con, &mut panel, &objects, &map, &mut fov_map,
               recompute_fov);

    if game_state.debug_mode {
      let mut seed_type_label = "Active";
      if thread_ctx.custom_seed {
        root.set_default_foreground(colors::RED);
        seed_type_label = "Custom";
      }
      else {
        root.set_default_foreground(colors::WHITE);
      }
      root.print_ex(1, SCREEN_HEIGHT - 2, BackgroundFlag::None, TextAlignment::Left,
                    format!("{} Seed: {}", seed_type_label, thread_ctx.rand_seed));
    }

    root.flush();
    root.clear(); // clears text

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

    let player_action = handle_input(&mut root, &map, &mut objects, &mut game_state);

    if player_action == PlayerAction::Exit || root.window_closed() {
      game_state.game_running = false;
    }

    // Update monsters
    if game_state.game_running && player_action == PlayerAction::TookTurn {
      for id in 0..objects.len() {
        if objects[id].brain.is_some() && objects[id].alive {
          ai_take_turn(id, &mut objects, &map, &mut fov_map, &mut game_state);
        }
      }
    }
  }
}
