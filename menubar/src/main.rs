mod icons;
mod state;
mod tray;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::TrayIconEvent;
use tray_icon::menu::MenuEvent;

use state::AppState;

enum UserEvent {
    TrayIcon(TrayIconEvent),
    Menu(MenuEvent),
}

fn main() {
    icons::validate();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Forward tray/menu events into the tao event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::TrayIcon(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    let current_state = AppState::Disconnected;
    let (_menu, menu_items) = tray::build_menu(&current_state);

    let mut tray_icon = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let (menu, _items) = tray::build_menu(&current_state);
                tray_icon = Some(tray::create_tray_icon(&current_state, menu));

                #[cfg(target_os = "macos")]
                {
                    use objc2_core_foundation::CFRunLoop;
                    let rl = CFRunLoop::main().unwrap();
                    rl.wake_up();
                }
            }

            Event::UserEvent(UserEvent::Menu(event)) => {
                if event.id == menu_items.quit.id() {
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => {}
        }
    });
}
