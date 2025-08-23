mod app;
mod core;
mod notifications;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

fn main() {
    app::startup::startup();
}
