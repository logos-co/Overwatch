# 🏓 Ping-Pong Example

A complete working example demonstrating the core features of the Overwatch framework.

---

## 🎯 What This Example Shows

```
┌──────────────────────────────────────────────────────────────────────┐
│                           OVERWATCH RUNNER                           │
│                                                                      │
│      ┌─────────────────┐                    ┌─────────────────┐      │
│      │  Ping Service   │                    │  Pong Service   │      │
│      │                 │ ── PongMessage ──> │                 │      │
│      │  • Sends Ping   │                    │  • Receives Ping│      │
│      │  • Counts Pongs │ <── PingMessage ── │  • Sends Pong   │      │
│      │  • Saves State  │                    │                 │      │
│      └─────────────────┘                    └─────────────────┘      │
│             │                                                        │
│             v                                                        │
│      ┌─────────────────┐                                             │
│      │    State File   │  (ping_state.json)                          │
│      │  { count: 15 }  │                                             │
│      └─────────────────┘                                             │
└──────────────────────────────────────────────────────────────────────┘
```

---

## ✨ Features Demonstrated

| Feature | File | Description |
|---------|------|-------------|
| **Service Definition** | `service_ping.rs`, `service_pong.rs` | How to create services with `ServiceData` and `ServiceCore` |
| **Message Passing** | `messages.rs` | Type-safe communication between services |
| **Settings** | `settings.rs` | Service configuration |
| **State Management** | `states.rs` | Persistent state that survives restarts |
| **State Operators** | `operators.rs` | Custom logic for saving/loading state |
| **Application Composition** | `main.rs` | Using `#[derive_services]` to wire everything |

---

## 🚀 Running the Example

```bash
# From the repository root
cargo run --example ping_pong
```

### Expected Output

```
Starting overwatch service
Sending Ping
Received Ping. Sending Pong.
Received Pong. Total: 1
Sending Ping
Received Ping. Sending Pong.
Received Pong. Total: 2
...
Received 30 Pongs. Exiting...
```

---

## 📁 File Structure

```
ping_pong/
├── Cargo.toml
├── README.md
├── saved_states/
│   └── ping_state.json    # Persisted state
└── src/
    ├── main.rs            # Application entry point
    ├── messages.rs        # Message type definitions
    ├── operators.rs       # State persistence logic
    ├── service_ping.rs    # Ping service implementation
    ├── service_pong.rs    # Pong service implementation
    ├── settings.rs        # Configuration structs
    └── states.rs          # State definitions
```

---

## 📝 Code Walkthrough

### 1. Define Messages (`messages.rs`)

```rust
#[derive(Debug)]
pub enum PingMessage {
    Pong,  // Ping receives Pong messages
}

#[derive(Debug)]
pub enum PongMessage {
    Ping,  // Pong receives Ping messages
}
```

### 2. Define Settings (`settings.rs`)

```rust
#[derive(Debug, Clone)]
pub struct PingSettings {
    pub state_save_path: String,
}
```

### 3. Define State (`states.rs`)

```rust
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PingState {
    pub pong_count: u32,
}

impl ServiceState for PingState {
    type Settings = PingSettings;
    type Error = PingStateError;
    
    fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}
```

### 4. Create State Operator (`operators.rs`)

```rust
#[async_trait]
impl StateOperator for StateSaveOperator {
    type State = PingState;
    
    fn try_load(settings: &Settings) -> Result<Option<Self::State>, Self::LoadError> {
        // Load from JSON file
        let state_string = std::fs::read_to_string(&settings.state_save_path)?;
        serde_json::from_str(&state_string).map_err(...)
    }
    
    async fn run(&mut self, state: Self::State) {
        // Save to JSON file
        let json = serde_json::to_string(&state).unwrap();
        std::fs::write(&self.save_path, json).unwrap();
    }
}
```

### 5. Implement Services (`service_ping.rs`)

```rust
impl ServiceData for PingService {
    type Settings = PingSettings;
    type State = PingState;
    type StateOperator = StateSaveOperator;
    type Message = PingMessage;
}

#[async_trait]
impl ServiceCore<RuntimeServiceId> for PingService {
    async fn run(self) -> Result<(), DynError> {
        // Get relay to Pong service
        let pong_relay = self.handle
            .overwatch_handle
            .relay::<PongService>()
            .await?;
        
        loop {
            tokio::select! {
                // Every second, send Ping
                () = sleep(Duration::from_secs(1)) => {
                    pong_relay.send(PongMessage::Ping).await?;
                }
                // Handle incoming Pong messages
                Some(PingMessage::Pong) = self.inbound_relay.recv() => {
                    self.pong_count += 1;
                    // Update state (triggers save)
                    self.state_updater.update(Some(PingState { 
                        pong_count: self.pong_count 
                    }));
                }
            }
        }
    }
}
```

### 6. Compose Application (`main.rs`)

```rust
#[derive_services]
struct PingPong {
    ping: PingService,
    pong: PongService,
}

fn main() {
    let settings = PingPongServiceSettings {
        ping: PingSettings { state_save_path: "...".into() },
        pong: (),
    };
    
    let app = OverwatchRunner::<PingPong>::run(settings, None)
        .expect("Failed to start");
    
    app.runtime()
        .handle()
        .block_on(app.handle().start_all_services())
        .expect("Failed to start services");
    
    app.blocking_wait_finished();
}
```

---

## 🔄 State Persistence

The Ping service demonstrates state persistence:

1. **On startup**: Tries to load state from `saved_states/ping_state.json`
2. **During runtime**: Updates count after each Pong
3. **On state update**: Automatically saves to JSON file

Try this:
```bash
# Run until a few pongs
cargo run --example ping_pong
# Ctrl+C to stop

# Check the saved state
cat examples/ping_pong/saved_states/ping_state.json
# {"pong_count":5}

# Run again - it will resume from where it left off!
cargo run --example ping_pong
```

---

## 🧪 Try Modifying It

1. **Change the interval**: Modify `Duration::from_secs(1)` in `service_ping.rs`
2. **Add a new message type**: Extend the `PingMessage` or `PongMessage` enums
3. **Add a third service**: Create a new service and add it to the `PingPong` struct
4. **Change state persistence**: Modify `StateSaveOperator` to use a database

---

## 📖 Learn More

- [Main README](../../README.md) - Framework overview
- [API Documentation](https://docs.rs/overwatch) - Full API reference