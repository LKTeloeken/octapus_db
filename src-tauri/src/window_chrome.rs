//! Barra de título customizada. No macOS a janela usa `titleBarStyle: Overlay`
//! (conteúdo até o topo, com cantos, sombra e tela cheia nativos) e o front
//! desenha o próprio semáforo — então os botões nativos são escondidos aqui.
//! No Linux a janela já nasce sem decoração (`tauri.linux.conf.json`).

use tauri::{Runtime, Window};

/// Esconde fechar/minimizar/zoom nativos. O AppKit recria a barra ao entrar e
/// sair da tela cheia, por isso isto roda de novo a cada redimensionamento.
#[cfg(target_os = "macos")]
pub fn hide_traffic_lights<R: Runtime>(window: &Window<R>) {
    use objc2_app_kit::{NSWindow, NSWindowButton};

    let Ok(ptr) = window.ns_window() else {
        return;
    };
    // SAFETY: `ns_window` devolve o NSWindow vivo desta janela; os eventos de
    // janela e o `setup` rodam na thread principal, exigida pelo AppKit.
    let ns_window = unsafe { &*ptr.cast::<NSWindow>() };

    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = ns_window.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_traffic_lights<R: Runtime>(_window: &Window<R>) {}
