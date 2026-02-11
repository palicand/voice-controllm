mod bridge;
mod client;
mod icons;
mod paths;
mod state;
mod tray;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::TrayIconEvent;
use tray_icon::menu::MenuEvent;

use bridge::{AppEvent, Command, UserEvent};
use state::AppState;

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

    // Spawn async runtime on background thread
    let cmd_tx = bridge::spawn_async_runtime(event_loop.create_proxy());

    let mut current_state = AppState::Disconnected;
    let (_menu, mut menu_items) = tray::build_menu(&current_state);

    let mut tray_icon = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let (menu, items) = tray::build_menu(&current_state);
                menu_items = items;
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
                    let _ = cmd_tx.send(Command::Shutdown);
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                } else if event.id == menu_items.toggle.id() {
                    match current_state {
                        AppState::Listening => {
                            let _ = cmd_tx.send(Command::StopListening);
                        }
                        AppState::Paused => {
                            let _ = cmd_tx.send(Command::StartListening);
                        }
                        _ => {}
                    }
                }
            }

            Event::UserEvent(UserEvent::App(AppEvent::StateChanged(new_state))) => {
                current_state = new_state;
                if let Some(ref ti) = tray_icon {
                    let (new_menu, new_items) = tray::build_menu(&current_state);
                    menu_items = new_items;
                    ti.set_menu(Some(Box::new(new_menu)));
                    ti.set_icon(Some(tray::select_icon_for_state(&current_state)))
                        .expect("failed to set icon");
                }
            }

            _ => {}
        }
    });
}
