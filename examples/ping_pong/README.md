# PingPong

This example project demonstrates how to set up a basic Overwatch application in Rust.

### Behaviour

This project demonstrates a simple communication pattern between two services.

1. Every second, the `Ping` service sends a message to the `Pong` service.
2. The `Pong` service receives the message and prints it to the console. Afterwards, it sends a message back to the
   `Ping` service.
3. The `Ping` service receives the message and prints it to the console.
    - After each received `Pong` message, a counter is incremented and saved into a file from which it can be restored
      when the application is restarted.

### Features

- **Services**: Shows how to define and register services within an Overwatch application.
- **Messages**: Demonstrates communication between services using the relay.
- **Settings**: Shows how to define and access settings within an Overwatch application.
- **States**: Demonstrates how to use the state to store data within an Overwatch application.
- **StateOperators**: Shows how to use the state operator to interact with the state.
    - In this example, in combination with **State**, it shows how to save and load data from the state.

### About

This project serves as a barebones template to get started with the Overwatch library.
It provides the foundational structure for creating more complex applications.

