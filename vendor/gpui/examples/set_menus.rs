use gpui::{
    App, Application, Context, Menu, MenuItem, SystemMenuType, Window, WindowOptions, actions, div,
    prelude::*, rgb,
};

struct SetMenus;

impl Render for SetMenus {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x2e7d32))
            .size_full()
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0xffffff))
            .child("Set Menus Example")
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        cx.on_action(quit);
        cx.set_menus(vec![Menu {
            name: "set_menus".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        }]);
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| SetMenus {}))
            .unwrap();
    });
}

actions!(set_menus, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    println!("Gracefully quitting the application . . .");
    cx.quit();
}
