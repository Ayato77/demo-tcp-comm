use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use std::error::Error;
use tokio::spawn;
use tokio::time::{self, Duration};

async fn handle_peer(socket: TcpStream, mut rx: mpsc::Receiver<String>) -> io::Result<()> {
    // Use `split()` to get mutable references for reading and writing
    let (mut reader, mut writer) = io::split(socket);
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    //let mut line = String::new();

    // Spawn a task to write messages from the receiver to the writer
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if writer.write_all(msg.as_bytes()).await.is_err() {
                break;
            }
            // Flush the writer to send the message immediately
            if writer.flush().await.is_err() {
                eprintln!("Failed to flush writer");
                break;
            }
        }
    });

    // Main task to read from the socket
    loop {
        buf.clear();
        let bytes_read = reader.read_until(b',', &mut buf).await?;
        if bytes_read == 0 {
            break; // Connection closed
        }

        let line = String::from_utf8_lossy(&buf);
        println!("Received: {}", line.trim());
    }

    Ok(())
}

// send simple message periodically
//TODO: create multiple nodes and let them communicate with each other(at least 3 nodes, Adaptation Manager + two nodes)
//TODO: Implement random load shedding
async fn send_msg_every_five_sec(tx: mpsc::Sender<String>) {
    let key = String::from("temperature");
    let mut value = 20.0;

    loop {
        let msg = format!("{};{};end,", key, value);
        println!("Send: {}", msg);
        // Try sending the message through the channel
        if tx.send(msg.clone()).await.is_err() {
            eprintln!("Error: Failed to send message.");
            break;
        }

        // Simulate increasing the value for the next message
        value += 1.0;

        // Wait for 5 seconds before sending the next message
        time::sleep(Duration::from_secs(5)).await;
    }
}

// Function for connecting a peer and handle communication
async fn connect_to_peer(addr: &str, tx: mpsc::Sender<String>) -> io::Result<TcpStream> {
    let socket = TcpStream::connect(addr).await?;

    // Use tokio::spawn_blocking to handle stdin input, since stdin may block in async context.
    /*tokio::task::spawn_blocking(move || {
        let mut input = String::new();
        let stdin = std::io::stdin();

        loop {
            input.clear();

            // Blocking read from stdin
            if stdin.read_line(&mut input).is_err() {
                eprintln!("Error reading from stdin.");
                break;
            }

            let trimmed_input = input.trim().to_string()+",";

            // Log the input
            eprintln!("Read input: {:?}", trimmed_input);

            // Send the message to the async task via the mpsc channel
            if tx.blocking_send(trimmed_input).is_err() {
                eprintln!("Failed to send message. Receiver may have been dropped.");
                break;
            }
        }
    });*/

    spawn(send_msg_every_five_sec(tx));

    Ok(socket)  // Return the socket for further communication
}

// Function to start a P2P listener
async fn start_listener(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on: {}", addr);

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        println!("Peer connected: {}", peer_addr);

        // If mpsc channel is full, sender waits for a space
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            if let Err(e) = handle_peer(socket, rx).await {
                eprintln!("Error handling peer: {}", e);
            }
        });

    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "listener".to_string());
    //TODO: Can we get addresses as list from CLI??
    let addr = std::env::args().nth(2).unwrap_or_else(|| "127.0.0.1:8080".to_string());

    if mode == "listener" {
        start_listener(&addr).await?;
    } else {
        // Connect to a peer and start bi-directional communication
        //TODO: connect to multiple nodes
        let (tx, rx) = mpsc::channel(32);
        let socket = connect_to_peer(&addr, tx).await?;
        handle_peer(socket, rx).await?;
    }

    Ok(())
}
