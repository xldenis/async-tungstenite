use async_tungstenite::{gio::connect_async, tungstenite::Message};
use futures::prelude::*;

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let url = url::Url::parse("wss://echo.websocket.org").unwrap();

    let (mut ws_stream, _) = connect_async(url).await?;

    let text = "Hello, World!";

    println!("Sending: \"{}\"", text);
    ws_stream.send(Message::text(text)).await?;

    let msg = ws_stream
        .next()
        .await
        .ok_or_else(|| "didn't receive anything")??;

    println!("Received: {:?}", msg);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the default main context and run our async function on it
    let main_context = glib::MainContext::default();
    main_context.block_on(run())
}
