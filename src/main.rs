extern crate tcod;

use std::cmp;
use tcod::console::*;
use tcod::colors::{self, Color};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 30;

const MAP_WIDTH: i32 = SCREEN_WIDTH;
const MAP_HEIGHT: i32 = SCREEN_HEIGHT - 5;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100, };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150, };


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
}


#[derive(Clone, Copy, Debug)]
struct Tile {
    // @future try using Object for tiles. Can then reuse HP, damage given, etc.
    passable: bool,
    block_sight: bool
}

impl Tile {
    pub fn empty() -> Self {
        Tile { passable: true, block_sight: false }
    }

    pub fn wall() -> Self {
        Tile { passable: false, block_sight: true }
    }

    pub fn make_empty(tile: &mut Tile) {
        tile.passable = true;
        tile.block_sight = false;
    }
}

type Map = Vec<Tile>;

/* Places a rect of empty tiles into `map` */
fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
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

fn make_map() -> Map {
    let mut map = vec![Tile::wall(); (MAP_WIDTH * MAP_HEIGHT) as usize];

    let room1 = Rect::new(20, 15, 10, 15);
    let room2 = Rect::new(50, 15, 10, 15);
    create_room(room1, &mut map);
    create_room(room2, &mut map);

    create_h_tunnel(25, 55, 23, &mut map);

    map
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

fn render_all(root: &mut Root, con: &mut Offscreen, objects: &[Object], map: &Map) {
    for x in 0..MAP_WIDTH {
        for y in 0..MAP_HEIGHT {
            let is_wall = map[(y * MAP_WIDTH + x) as usize].block_sight;
            if is_wall {
                con.set_char_background(x, y, COLOR_DARK_WALL, BackgroundFlag::Set);
            } else {
                con.set_char_background(x, y, COLOR_DARK_GROUND, BackgroundFlag::Set);
            }
        }
    }

    for object in objects {
        object.draw(con);
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

    let mut con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);

    tcod::system::set_fps(LIMIT_FPS);

    let player = Object::new(25, 23, '@', colors::WHITE);
    let wizard = Object::new(55, 28, '@', colors::YELLOW);

    let mut objects = [player, wizard];

    let map = make_map();

    while !root.window_closed() {
        render_all(&mut root, &mut con, &objects, &map);

        root.flush();

        // Erase objects at their old locations before moving
        for object in &objects {
            object.clear(&mut con);
        }

        let player = &mut objects[0];
        let exit = handle_input(&mut root, player, &map);
        if exit {
            break;
        }
    }
}
