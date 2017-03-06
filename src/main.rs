extern crate tcod;

use tcod::console::*;
use tcod::colors;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 30;

fn main() {
    let mut root = Root::initializer()
        .font("data/fonts/arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rusty Roguelike")
        .init();

    tcod::system::set_fps(LIMIT_FPS);

    while !root.window_closed() {
        root.set_default_foreground(colors::WHITE);
        root.put_char(1, 1, '@', BackgroundFlag::None);
        root.flush();
        root.wait_for_keypress(true);
    }
}
