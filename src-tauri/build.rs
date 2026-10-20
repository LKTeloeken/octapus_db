fn main() {
    // URL da API do assistente embutida no build (`assistant/config.rs`, via
    // `option_env!`): trocar a variável recompila o app.
    println!("cargo:rerun-if-env-changed=OCTAPUS_ASSISTANT_URL");
    tauri_build::build()
}
