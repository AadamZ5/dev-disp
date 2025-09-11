use std::{fs::File, io::Read, os::fd::FromRawFd, thread};

use dev_disp_usb_proto_android::MessageToAndroid;

pub enum IncomingMessage {
    ScreenUpdate(Vec<u8>),
    GetScreenInfo,
    Quit,
}

impl From<MessageToAndroid> for IncomingMessage {
    fn from(msg: MessageToAndroid) -> Self {
        match msg {
            MessageToAndroid::ScreenUpdate(m) => IncomingMessage::ScreenUpdate(m.payload),
            MessageToAndroid::GetScreenInfo(_) => IncomingMessage::GetScreenInfo,
            MessageToAndroid::Quit(_) => IncomingMessage::Quit,
        }
    }
}

pub fn initialize(fd: i32) -> Result<(Vec<IncomingMessage>, Vec<u8>), String> {
    let mut file = unsafe { File::from_raw_fd(fd) };
    // Use the file as needed
    println!("Initialized with file descriptor: {}", fd);

    let mut buffer = Vec::with_capacity(1024); // 1 MB buffer

    // Read from the file and try to deserialize messages
    let mut messages: Vec<IncomingMessage> = Vec::new();

    // Continously read from the file until the buffer is full
    let mut msg_buffer = [0u8; 256];
    while buffer.len() < buffer.capacity() {
        match file.read(&mut msg_buffer) {
            Ok(bytes_read) => {
                println!("Read {} bytes from file descriptor {}", bytes_read, fd);
                if bytes_read == 0 {
                    // Wait for more data
                    thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                if (buffer.len() + bytes_read) > buffer.capacity() {
                    println!("Buffer full, stopping read.");
                    break;
                }
                buffer.extend_from_slice(&msg_buffer[..bytes_read]);
                println!("Data: {:?}", &buffer);
            }
            Err(e) => {
                return Err(format!("Failed to read from file descriptor {}: {}", fd, e));
            }
        }

        match MessageToAndroid::deserialize(&buffer) {
            Ok((msg, size)) => {
                println!("Deserialized MessageToAndroid: {:?}, size: {}", msg, size);
                messages.push(msg.into());
                // Remove the processed bytes from the buffer
                buffer.drain(0..size);
            }
            Err(e) => {
                println!("Failed to deserialize MessageToAndroid: {}", e);
                continue;
            }
        }
    }

    drop(file);
    println!("File descriptor {} has been dropped.", fd);
    Ok((messages, buffer))
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
