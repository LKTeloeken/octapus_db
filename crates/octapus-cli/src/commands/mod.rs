//! Os subcomandos do `octapus`. Cada um abre a conexão que precisa, chama o
//! núcleo e entrega o resultado à saída — sem lógica de banco aqui.

pub mod group;
pub mod query;
pub mod server;
pub mod structure;
pub mod table;
