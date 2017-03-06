extern crate tcod;

use tcod::console::*;
use tcod::colors;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 30;

fn handle_input(root: &mut Root, player_x: &mut i32, player_y: &mut i32) -> bool {
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
        Key { code: Up, .. } => *player_y -= 1,
        Key { code: Down, .. } => *player_y += 1,
        Key { code: Left, .. } => *player_x -= 1,
        Key { code: Right, .. } => *player_x += 1,

        _ => {}
    }
    false
}

fn main() {
    let mut root = Root::initializer()
        .font("data/fonts/arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rusty Roguelike")
        .init();

    tcod::system::set_fps(LIMIT_FPS);

    let mut player_x = SCREEN_WIDTH / 2;
    let mut player_y = SCREEN_HEIGHT / 2;

    while !root.window_closed() {
        root.set_default_foreground(colors::WHITE);
        root.put_char(player_x, player_y, '@', BackgroundFlag::None);

        root.flush();

        // Clear the previous player position
        root.put_char(player_x, player_y, ' ', BackgroundFlag::None);

        let exit = handle_input(&mut root, &mut player_x, &mut player_y);
        if exit {
            break;
        }
    }
}
