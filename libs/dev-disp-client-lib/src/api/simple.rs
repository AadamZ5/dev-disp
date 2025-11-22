use std::{fs::File, io::Read, os::fd::FromRawFd, sync::Mutex, thread};

use dev_disp_comm::usb::strategies::android_aoa::protocol::MessageToAndroid;
use log::{info, warn};
use once_cell::sync::Lazy;

use crate::frb_generated::StreamSink;

static SINK_GET_SCREEN: Lazy<Mutex<Option<StreamSink<bool>>>> = Lazy::new(|| Mutex::new(None));

pub fn listen_get_screen(sink: StreamSink<bool>) -> Result<(), String> {
    info!("Setting up SINK_GET_SCREEN");
    let sink_get_screen_cell = &SINK_GET_SCREEN;
    let mut guard = sink_get_screen_cell
        .lock()
        .map_err(|e| format!("Failed to lock SINK_GET_SCREEN: {}", e))?;
    *guard = Some(sink);
    info!("SINK_GET_SCREEN is now set up");
    Ok(())
}

pub fn initialize(fd: i32) -> Result<(), String> {
    let mut file = unsafe { File::from_raw_fd(fd) };
    // Use the file as needed
    info!("Initialized with file descriptor: {}", fd);

    // Continously read from the file until the buffer is full
    // TODO: We will need to increase this size once we start sending frames
    let mut buffer = Vec::with_capacity(8294407 + 4096); // Enough for 1920*1080*4 and some x-tra space
    let mut msg_buffer = [0u8; 256];
    loop {
        match file.read(&mut msg_buffer) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    // Wait for more data
                    thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                buffer.extend_from_slice(&msg_buffer[..bytes_read]);

                loop {
                    let mut consumed = 0;
                    match process_buffer(&buffer[consumed..]) {
                        Ok((msg, msg_consumed)) => {
                            consumed = msg_consumed;
                            buffer.drain(0..consumed);
                            handle_message(msg)?;
                        }
                        Err(_) => {
                            // Not enough data to form a complete message
                            break;
                        }
                    }

                    if consumed <= 0 {
                        break;
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to read from file descriptor {}: {}", fd, e));
            }
        }
    }

    Ok(())
}

fn process_buffer(msg_buffer: &[u8]) -> Result<(MessageToAndroid, usize), String> {
    MessageToAndroid::deserialize(&msg_buffer)
        .map_err(|e| format!("Failed to decode message from Android: {}", e))
}

fn handle_message(msg: MessageToAndroid) -> Result<(), String> {
    info!("Received message from Android: {:?}", msg);
    match msg {
        MessageToAndroid::GetScreenInfo(_) => {
            let sink_get_screen_cell = &SINK_GET_SCREEN;
            let guard = sink_get_screen_cell
                .lock()
                .map_err(|e| format!("Failed to lock SINK_GET_SCREEN: {}", e))?;
            if let Some(sink) = &*guard {
                if let Err(e) = sink.add(true) {
                    return Err(format!("Failed to send screen info: {}", e));
                }
            } else {
                return Err("SINK_GET_SCREEN is not initialized".to_string());
            }
        }
        MessageToAndroid::ScreenUpdate(data) => {
            info!("Received screen update of size: {}", data.payload.len());
            // Here you would typically process the screen data, e.g., update a UI component
        }
        _ => {
            return Err(format!("Unhandled message: {:?}", msg));
        }
    }
    Ok(())
}

pub enum MessageToDart {
    GetScreenInfo,
    ScreenUpdate(Vec<u8>),
}

impl TryFrom<MessageToAndroid> for MessageToDart {
    type Error = String;

    fn try_from(value: MessageToAndroid) -> Result<Self, Self::Error> {
        match value {
            MessageToAndroid::GetScreenInfo(_) => Ok(MessageToDart::GetScreenInfo),
            MessageToAndroid::ScreenUpdate(data) => Ok(MessageToDart::ScreenUpdate(data.payload)),
            _ => Err(format!("Cannot convert {:?} to MessageToDart", value)),
        }
    }
}

pub fn initialize_streaming(fd: i32, sink: StreamSink<MessageToDart>) -> Result<(), String> {
    let mut file = unsafe { File::from_raw_fd(fd) };
    // Use the file as needed
    info!("Initialized streaming with file descriptor: {}", fd);

    // Continously read from the file until the buffer is full
    // TODO: We will need to increase this size once we start sending frames
    let mut buffer = Vec::with_capacity(8294407 + 4096); // Enough for 1920*1080*4 and some x-tra space
    let mut msg_buffer = [0u8; 256];
    loop {
        match file.read(&mut msg_buffer) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    // Wait for more data
                    thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                buffer.extend_from_slice(&msg_buffer[..bytes_read]);

                loop {
                    let mut consumed = 0;
                    match process_buffer(&buffer[consumed..]) {
                        Ok((msg, msg_consumed)) => {
                            consumed = msg_consumed;
                            buffer.drain(0..consumed);

                            let send_result = MessageToDart::try_from(msg).and_then(|dart_msg| {
                                sink.add(dart_msg)
                                    .map_err(|e| format!("Failed to send message to Dart: {}", e))
                            });

                            if let Err(e) = send_result {
                                warn!("Failed to send message to Dart: {}", e);
                            }
                        }
                        Err(_) => {
                            // Not enough data to form a complete message
                            break;
                        }
                    }

                    if consumed <= 0 {
                        break;
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to read from file descriptor {}: {}", fd, e));
            }
        }
    }
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
