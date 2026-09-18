//! This module exposes a gRPC API "client" for connecting to the display control daemon.
//! It provides a factory for creating and managing gRPC client instances.

mod grpc_api_factory;

pub use grpc_api_factory::*;
