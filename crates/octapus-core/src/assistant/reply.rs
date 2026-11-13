//! A resposta do usuário a uma pergunta do modelo (`kind: ask_user`): o turno
//! abre a espera antes de mostrar a pergunta, e o `assistant_turn_reply` da
//! tela entrega a resposta. Uma espera por vez; resposta sem ninguém
//! esperando é recusada (não fica guardada para a próxima pergunta).

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::oneshot;

#[derive(Clone, Default)]
pub struct ReplySlot(Arc<Mutex<Option<oneshot::Sender<String>>>>);

impl ReplySlot {
    /// Abre a espera e devolve onde a resposta vai chegar. Abrir de novo
    /// descarta a espera anterior.
    pub fn open(&self) -> oneshot::Receiver<String> {
        let (sender, receiver) = oneshot::channel();
        *self.0.lock() = Some(sender);
        receiver
    }

    /// Entrega a resposta; `false` se o turno não está esperando uma.
    pub fn reply(&self, text: String) -> bool {
        match self.0.lock().take() {
            Some(sender) => sender.send(text).is_ok(),
            None => false,
        }
    }

    /// Encerra a espera sem resposta (cancelamento, tempo esgotado).
    pub fn close(&self) {
        self.0.lock().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn delivers_only_while_open() {
        let slot = ReplySlot::default();
        assert!(!slot.reply("cedo".into()), "sem espera, recusa");

        let waiting = slot.open();
        assert!(slot.reply("outubro".into()));
        assert_eq!(waiting.await.unwrap(), "outubro");
        assert!(!slot.reply("de novo".into()), "a espera fecha com a resposta");

        let waiting = slot.open();
        slot.close();
        assert!(waiting.await.is_err());
        assert!(!slot.reply("tarde".into()));
    }
}
