mod app;
mod common;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

fn main() {
    app::startup::startup();
}
