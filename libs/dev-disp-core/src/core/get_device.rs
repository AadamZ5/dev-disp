use std::fmt::Display;

use evdi::device_node::DeviceNode;
use log::{debug, error, info};
use thiserror::Error;

#[derive(Error, Debug)]
pub struct NoDeviceError;
impl Display for NoDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No evdi device node found and failed to create one")
    }
}

pub fn get_device() -> Result<DeviceNode, NoDeviceError> {
    DeviceNode::get()
        .or_else(|| {
            debug!("Failed to get an existing device node, will try to create one");
            if DeviceNode::add() {
                debug!("Successfully added a device node");
                DeviceNode::get().or_else(|| {
                    error!("Added a device node but still can't get it!");
                    None
                })
            } else {
                error!("Failed to add a device node, do you have superuser permissions?");
                None
            }
        })
        .ok_or(NoDeviceError)
}
