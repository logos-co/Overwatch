# PingPong
This example project demonstrates how to set up a basic Overwatch application in Rust.

### Behaviour
This project demonstrates a simple communication pattern between two services.
1. Every second, the `Ping` service sends a message to the `Pong` service.
2. The `Pong` service receives the message and prints it to the console. Afterwards, it sends a message back to the `Ping` service.
3. The `Ping` service receives the message and prints it to the console.

### Features
- **Services**: Shows how to define and register services within an Overwatch application.
- **Message Passing**: Demonstrates communication between services using the relay.

### About
This project serves as a barebones template to get started with the Overwatch library. 
It provides the foundational structure for creating more complex applications.

