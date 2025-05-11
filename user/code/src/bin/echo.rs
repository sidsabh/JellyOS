#![feature(never_type)]
#![no_std]
#![no_main]

use core::time::Duration;

use user::*;

use kernel_api::{syscall::{exit, sleep, sock_create, sock_listen, sock_recv, sock_send, sock_status}, OsResult};

#[no_mangle]
fn main(argc: usize, argv_ptr: *const *const u8) {
    let result = main_inner();
    if result.is_err() {
        println!("Terminating with error: {:?}", result);
    }
}

fn main_inner() -> OsResult<!> {
    let sock = sock_create();
    sock_listen(sock, 80)?;
    println!("Listening on port 80");
    loop {
        
        
        // Wait till a connection is available
        loop {
            let status = sock_status(sock)?;
            if !status.can_send {
                break;
            } else {
                println!("Waiting for a connection...");
                sleep(Duration::from_secs(1))?;
            }
        }

        // Send a welcome message
        let welcome_message = b"Welcome to JellyOS echo server!\n";
        let mut bytes_sent = 0;
        while bytes_sent < welcome_message.len() {
            let result = sock_send(sock, &welcome_message[bytes_sent..])?;
            if result == 0 {
                break;
            }
            bytes_sent += result;
        }

        // Inside another loop, receive a packet and send it back through the socket. Also print the message to the console with print!().
        loop {
            let mut buffer = [0u8; 1024];
            let bytes_read = sock_recv(sock, &mut buffer)?;
            if bytes_read == 0 {
                continue;
            }
            let message = core::str::from_utf8(&buffer[..bytes_read]).unwrap_or("Invalid UTF-8");
            println!("Received: {}", message);
            let mut bytes_sent = 0;
            while bytes_sent < bytes_read {
                let result = sock_send(sock, &buffer[bytes_sent..])?;
                if result == 0 {
                    break;
                }
                bytes_sent += result;
            }
            println!("Sent: {}", core::str::from_utf8(&buffer[..bytes_read]).unwrap_or("Invalid UTF-8"));
            
            // Check if the message is "exit" to terminate the server
            if message.trim() == "exit" {
                println!("Exiting echo server...");
                exit();
            }
        }
    }
}
