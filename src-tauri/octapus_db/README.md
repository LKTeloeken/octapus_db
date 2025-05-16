# Octapus DB

Octapus DB is a Rust-based application designed to manage connections to external databases, specifically PostgreSQL, using a lightweight SQLite database to store connection information. This project provides a structured way to execute queries on various databases while maintaining efficient connection management.

## Features

- Manage connections to PostgreSQL databases.
- Store server connection details in an SQLite database.
- Execute queries on external databases using a connection pool.
- Retrieve database schemas, tables, and columns dynamically.

## Project Structure

```
octapus_db
├── src-tauri
│   ├── src
│   │   ├── commands
│   │   │   ├── connection_commands.rs  # Manages connections to external databases
│   │   │   ├── postgre_commands.rs      # Executes queries on PostgreSQL databases
│   │   │   └── mod.rs
│   │   ├── db_manager
│   │   │   ├── connection_pool.rs       # Implements a connection pool for database connections
│   │   │   ├── query_executor.rs         # Executes queries using the connection pool
│   │   │   └── mod.rs
│   │   ├── models
│   │   │   ├── connection.rs             # Defines the connection details struct
│   │   │   └── mod.rs
│   │   ├── app_state.rs                  # Manages application state and SQLite connection
│   │   ├── db.rs                         # Initializes the SQLite database and creates tables
│   │   └── main.rs                       # Entry point of the application
│   ├── Cargo.toml                        # Project dependencies and configuration
│   └── tauri.conf.json                   # Tauri application configuration
└── README.md                             # Project documentation
```

## Setup Instructions

1. **Clone the repository:**
   ```bash
   git clone <repository-url>
   cd octapus_db
   ```

2. **Build the project:**
   Ensure you have Rust and Cargo installed. Then run:
   ```bash
   cargo build
   ```

3. **Run the application:**
   ```bash
   cargo run
   ```

## Usage Guidelines

- Configure your PostgreSQL server details in the SQLite database using the provided commands.
- Use the connection commands to establish connections and execute queries on your PostgreSQL databases.
- Refer to the individual command files for specific functionalities and examples.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any enhancements or bug fixes.

## License

This project is licensed under the MIT License. See the LICENSE file for more details.