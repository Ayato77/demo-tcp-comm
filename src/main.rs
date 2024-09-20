use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use std::error::Error;
use tokio::spawn;
use tokio::time::{self, Duration};
use clap::Parser;
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Parser, Debug)]
struct Cli {
    // The role which the node takes (listener, sender)
    #[clap(short, long)]
    role: String,
    // Addresses of peers (supports multiple addresses for sender mode)
    #[arg(short, long, required = true)]
    address: Vec<String>,
}

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
    println!("Connected to: {}", addr);

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

        //TODO: create a channel with a small buffer for each connection. Use hash map to store the corresponding tx channel.

        spawn(async move {
            if let Err(e) = handle_peer(socket, rx).await {
                eprintln!("Error handling peer: {}", e);
            }
        });

        spawn(send_msg_every_five_sec(tx));

    }
}

#[tokio::main]
async fn main() {
    // Parse CLI arguments
    let args = Cli::parse();

    if args.address.is_empty() {
        eprintln!("Please provide at least one address as argument");
    }

    if args.role == "listener" {
        if args.address.len() > 1 {
            eprintln!("Error: Multiple addresses for listener are given!!");
        }
        if start_listener(&args.address[0]).await.is_err(){
            eprintln!("Failed: start listener");
        }
    } else {
        println!("Start sender");
        // Use Arc<Mutex<HashMap>> for shared and concurrent access
        let addr_channels = Arc::new(Mutex::new(HashMap::new()));

        for addr in &args.address {
            let addr_clone = addr.clone();
            let addr_channels_clone = Arc::clone(&addr_channels);

            // Create a new channel for each address
            let (tx, mut rx) = mpsc::channel(32);

            // Insert the sender into the hashmap inside a mutex
            {
                let mut addr_channels = addr_channels_clone.lock().await;
                addr_channels.insert(addr_clone.clone(), tx);
            }
            // Lock the hashmap and retrieve the sender for this address
            let addr_channels = addr_channels_clone.lock().await;
            if let Some(tx) = addr_channels.get(&addr_clone) {
                let socket = connect_to_peer(&addr_clone, tx.clone()).await.unwrap(); //TODO: Implement Error Handling and make sure that other healthy connections continues to run.
                spawn(async move{
                    if handle_peer(socket, rx).await.is_err(){
                        eprintln!("handle_peer failed");
                    }
                });
            } else {
                eprintln!("No sender found for {}", addr_clone);
            }
        }
    }

    //TODO: Make sure that this main function does not close without this loop!
    loop {

    }

}
