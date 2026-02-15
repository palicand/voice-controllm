mod bridge;
mod icons;
mod state;
mod tray;

use std::sync::mpsc;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::TrayIconEvent;
use tray_icon::menu::MenuEvent;

use bridge::{AppEvent, Command, UserEvent};
use state::AppState;

struct App {
    current_state: AppState,
    tray_icon: Option<tray_icon::TrayIcon>,
    menu_items: tray::MenuItems,
    cmd_tx: mpsc::Sender<Command>,
    shutting_down: bool,
}

impl App {
    fn new(cmd_tx: mpsc::Sender<Command>) -> Self {
        let state = AppState::Disconnected;
        let (_menu, menu_items) = tray::build_menu(&state);
        Self {
            current_state: state,
            tray_icon: None,
            menu_items,
            cmd_tx,
            shutting_down: false,
        }
    }

    fn handle_event(&mut self, event: Event<UserEvent>) -> ControlFlow {
        match event {
            Event::NewEvents(tao::event::StartCause::Init) => self.handle_init(),
            Event::UserEvent(UserEvent::Menu(event)) => self.handle_menu_event(event),
            Event::UserEvent(UserEvent::App(app_event)) => {
                return self.handle_app_event(app_event);
            }
            _ => {}
        }
        ControlFlow::Wait
    }

    fn handle_init(&mut self) {
        let (menu, items) = tray::build_menu(&self.current_state);
        self.menu_items = items;
        self.tray_icon = Some(tray::create_tray_icon(&self.current_state, menu));

        #[cfg(target_os = "macos")]
        {
            use objc2_core_foundation::CFRunLoop;
            let rl = CFRunLoop::main().unwrap();
            rl.wake_up();
        }
    }

    fn handle_menu_event(&mut self, event: MenuEvent) {
        if event.id == self.menu_items.quit.id() && !self.shutting_down {
            self.shutting_down = true;
            let _ = self.cmd_tx.send(Command::Shutdown);
        } else if event.id == self.menu_items.toggle.id() {
            match self.current_state {
                AppState::Listening => {
                    let _ = self.cmd_tx.send(Command::StopListening);
                }
                AppState::Paused => {
                    let _ = self.cmd_tx.send(Command::StartListening);
                }
                _ => {}
            }
        }
    }

    fn handle_app_event(&mut self, event: AppEvent) -> ControlFlow {
        match event {
            AppEvent::ShutdownRequested => {
                if !self.shutting_down {
                    self.shutting_down = true;
                    let _ = self.cmd_tx.send(Command::Shutdown);
                }
            }
            AppEvent::ShutdownComplete => {
                self.tray_icon.take();
                return ControlFlow::Exit;
            }
            AppEvent::StateChanged(new_state) => {
                self.current_state = new_state;
                if let Some(ref ti) = self.tray_icon {
                    let (new_menu, new_items) = tray::build_menu(&self.current_state);
                    self.menu_items = new_items;
                    ti.set_menu(Some(Box::new(new_menu)));
                    ti.set_icon(Some(tray::select_icon_for_state(&self.current_state)))
                        .expect("failed to set icon");
                }
            }
        }
        ControlFlow::Wait
    }
}

fn main() {
    icons::validate();

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Hide from Dock — must be set before run(), tao applies it during launch
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

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

    let mut app = App::new(cmd_tx);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = app.handle_event(event);
    });
}
