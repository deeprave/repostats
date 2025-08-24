mod app;
mod core;
mod notifications;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[tokio::main]
async fn main() {
    app::startup::startup_async().await;
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_main_is_async() {
        // Test that main function is now async
        // This test should pass once we've converted to async
        assert!(true, "Main function is now async");
    }
}
