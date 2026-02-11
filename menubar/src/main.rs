mod icons;

use tao::event_loop::EventLoopBuilder;

fn main() {
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = tao::event_loop::ControlFlow::Wait;

        if let tao::event::Event::NewEvents(tao::event::StartCause::Init) = event {
            println!("Menu bar app started");
        }
    });
}
