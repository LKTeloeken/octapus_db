//! O pedido de parada de um turno (C-18): o botão cancelar marca, e quem está
//! esperando (o stream da tentativa, a execução no banco) acorda na hora.

use std::sync::Arc;

use tokio::sync::watch;

#[derive(Clone)]
pub struct CancelFlag(Arc<watch::Sender<bool>>);

impl Default for CancelFlag {
    fn default() -> Self {
        Self(Arc::new(watch::Sender::new(false)))
    }
}

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }

    /// Termina quando o turno for cancelado (na hora, se já foi).
    pub async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        // `wait_for` olha o valor atual antes de esperar: não perde um
        // cancelamento que chegou antes
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn wakes_waiters_and_remembers() {
        let flag = CancelFlag::default();
        assert!(!flag.is_cancelled());

        let waiter = {
            let flag = flag.clone();
            tokio::spawn(async move { flag.cancelled().await })
        };
        tokio::time::sleep(Duration::from_millis(10)).await;
        flag.cancel();
        tokio::time::timeout(Duration::from_secs(1), waiter).await.unwrap().unwrap();

        // Quem chega depois não espera
        tokio::time::timeout(Duration::from_millis(50), flag.cancelled()).await.unwrap();
        assert!(flag.is_cancelled());
    }
}
