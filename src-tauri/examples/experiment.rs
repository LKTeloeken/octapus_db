//! Entrada do executor de rodadas do experimento do TCC (`src/experiment`).
//! É um example para não entrar no app nem no instalador:
//! `cargo run --example experiment --features experiment -- --help`.

fn main() {
    octapus_db_lib::experiment::main();
}
