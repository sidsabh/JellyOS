#![no_std]
#![no_main]

use user::*;
use kernel_api::syscall;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::alloc::format;

const MAX_LINE_LENGTH: usize = 512;
const ROOT_NAME: &str = "/";

const WELCOME_TXT: &str = r#"
     _      _ _        ___  ____  
    | | ___| | |_   _ / _ \/ ___| 
 _  | |/ _ \ | | | | | | | \___ \ 
| |_| |  __/ | | |_| | |_| |___) |
 \___/ \___|_|_|\__, |\___/|____/ 
                  |___/                     
"#;




fn normalize_path(path: &str) -> String {
    let mut components = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => continue, // Ignore empty components & `.`
            ".." => { components.pop(); } // Move up a directory
            _ => components.push(part),
        }
    }

    if components.is_empty() {
        "/".to_string() // If empty, return root
    } else {
        format!("/{}", components.join("/")) // Rebuild path
    }
}



#[no_mangle]
fn main() {
    println!("{}", WELCOME_TXT);

    let mut pwd = ROOT_NAME.to_string();
    let mut open_dirs: Vec<(String, usize)> = Vec::new(); // Cache opened directories

    // Open the root directory and cache its file descriptor
    if let Ok(fd) = syscall::open(ROOT_NAME) {
        open_dirs.push((pwd.clone(), fd));
    } else {
        println!("Error: Could not open root directory");
        return;
    }

    loop {
        print!("({}) $ ", pwd.to_uppercase());
        let mut line = Vec::new();

        loop {
            let mut byte_buf = [0u8; 1]; // Buffer to store a single byte
            let bytes_read = syscall::read(0, &mut byte_buf).unwrap_or(0);
            let b: u8 = if bytes_read > 0 { byte_buf[0] } else { 0 };

            if b == b'\r' || b == b'\n' {
                break;
            } else if b == 8 || b == 127 {
                if !line.is_empty() {
                    line.pop();
                    print!("\x08 \x08"); // Backspace effect
                }
            } else if b.is_ascii() && line.len() < MAX_LINE_LENGTH {
                line.push(b);
                print!("{}", b as char);
            } else {
                print!("\x07"); // Bell sound for invalid input
            }
        }
        println!("");

        let command_string = core::str::from_utf8(&line).unwrap_or("").trim();
        if command_string.is_empty() {
            continue;
        }

        let args: Vec<&str> = command_string.split_whitespace().collect();
        if args.is_empty() {
            continue;
        }

        match args[0] {
            "echo" => {
                println!("{}", args.iter().skip(1).cloned().collect::<Vec<_>>().join(" "));
            }
            "pwd" => {
                println!("{}", pwd.to_uppercase());
            }
            "cd" => {
                if args.len() > 1 {
                    let target = args[1];

                    let new_path = if target == ".." {
                        // If already at root, stay at root
                        if pwd == "/" {
                            "/".to_string()
                        } else {
                            // Split by `/`, remove last component, and reconstruct
                            let mut components: Vec<&str> = pwd.split('/').filter(|c| !c.is_empty()).collect();
                            components.pop(); // Remove last directory
                            if components.is_empty() {
                                "/".to_string() // If empty, stay at root
                            } else {
                                format!("/{}", components.join("/"))
                            }
                        }
                    } else if target.starts_with('/') {
                        // Absolute path
                        target.to_string()
                    } else {
                        // Relative path
                        format!("{}/{}", pwd.trim_end_matches('/'), target.trim_start_matches('/'))
                    };

                    // Normalize redundant `..` and `.` (like real UNIX systems)
                    let normalized_path = normalize_path(&new_path);

                    let fd = syscall::open(&normalized_path);
                    if let Ok(fd) = fd {
                        // If it's a directory, update `pwd` and cache it
                        pwd = normalized_path;
                        open_dirs.push((pwd.clone(), fd));
                    } else {
                        println!("error: directory {} not found", target.to_uppercase());
                    }
                }
            }

            "ls" => {
                let path = if args.len() > 1 {
                    let p = args[1];
                    if p.starts_with('/') {
                        p.to_string() // Absolute path
                    } else {
                        format!("{}/{}", pwd.trim_end_matches('/'), p.trim_start_matches('/')) // Relative path
                    }
                } else {
                    pwd.clone()
                };

                // Check if the directory is already open
                let fd = open_dirs.iter().find(|(p, _)| p == &path).map(|(_, fd)| *fd)
                    .or_else(|| syscall::open(&path).ok());

                if let Some(fd) = fd {
                    let mut buf = [0u8; 512];
                    let len = syscall::readdir(fd, &mut buf).unwrap_or_else(|_| {
                        println!("error: failed to read directory {}", path.to_uppercase());
                        0
                    });

                    if len > 0 {
                        println!("{}", core::str::from_utf8(&buf[..len]).unwrap_or("error reading dir"));
                    } else {
                        println!("error: directory {} is empty or could not be read", path.to_uppercase());
                    }

                    // Cache the directory if it wasn't already cached
                    if open_dirs.iter().all(|(p, _)| p != &path) {
                        open_dirs.push((path.clone(), fd));
                    }
                } else {
                    println!("error: directory {} not found", path.to_uppercase());
                }
            }

            "cat" => {
                for file in args.iter().skip(1) {
                    let file_path = format!("{}/{}", pwd, file);
                    if let Ok(fd) = syscall::open(&file_path) {
                        let mut buf = [0u8; 512];
                        let len = syscall::read(fd, &mut buf).unwrap_or(0);
                        println!("{}", core::str::from_utf8(&buf[..len]).unwrap_or("error reading file"));
                    } else {
                        println!("error: file {} not found", file.to_uppercase());
                    }
                }
            }
            "exit" => {
                syscall::exit();
            }
            "sleep" => {
                if args.len() == 2 {
                    if let Ok(ms) = args[1].parse::<u32>() {
                        let elapsed = syscall::sleep(core::time::Duration::from_millis(ms as u64)).unwrap_or_default();
                        println!("Slept for {:?}", elapsed);
                    }
                }
            }
            _ if args[0].starts_with("./") => {
                let path = args[0].to_string();
                let pid = syscall::fork();
                
                match pid {
                    Ok(0) => {
                        // Child process: Execute the new program
                        // drop first character of path
                        let path = &path[1..];
                        if syscall::exec(&path).is_err() {
                            println!("error: failed to execute {}", path);
                            syscall::exit();
                        }
                    }
                    Ok(_) => {
                        info!("created child process with PID {}", pid.unwrap());
                        // TODO: add child descriptors, wait, etc.
                        // syscall::wait();
                    }
                    Err(_) => {
                        println!("error: fork failed");
                    }
                }
            }
            _ => {
                println!("unknown command: {}", args[0]);
            }
        }
    }
}
