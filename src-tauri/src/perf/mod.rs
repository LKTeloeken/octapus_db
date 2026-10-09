//! Medições de desempenho do catálogo (Fase 0 do refactor do catálogo).
//!
//! Rodam contra os bancos descartáveis de `perf/catalog/` (portas 55432 e
//! 57017) — nunca contra um banco cadastrado no app. Todos os testes são
//! `#[ignore]`; para rodar só eles:
//!
//! ```text
//! OCTAPUS_PERF_TIER=S OCTAPUS_PERF_NET=local \
//!   cargo test --lib perf:: -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `OCTAPUS_PERF_NET=remote` passa as conexões por um proxy que simula rede
//! (RTT e banda em `OCTAPUS_PERF_RTT_MS` / `OCTAPUS_PERF_MBPS`). Cada medição
//! vira uma linha JSON em `perf/catalog/results/<suite>.jsonl`.

mod alloc_counter;
mod catalog_core;
mod fixture;
mod introspect;
mod latency_proxy;
mod mongo_baseline;
mod mongo_catalog;
mod postgres_baseline;
mod service;
