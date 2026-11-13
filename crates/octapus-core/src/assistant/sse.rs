//! Leitor de Server-Sent Events para o stream de tentativas
//! (`POST /v1/turns/{turnId}/attempts`).
//!
//! A API manda `event: <nome>\ndata: <json numa linha>\n\n`, mais linhas de
//! keepalive que começam com `:`. Os bytes chegam em pedaços de qualquer
//! tamanho (um evento pode vir partido, ou vários num pedaço só): o leitor
//! junta até a linha em branco que fecha cada evento.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Default)]
pub struct SseReader {
    buffer: Vec<u8>,
    event: Option<String>,
    data: Vec<String>,
}

impl SseReader {
    /// Os eventos completos que este pedaço fecha.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|&byte| byte == b'\n') {
            let raw: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&raw);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(event) = self.line(line) {
                events.push(event);
            }
        }
        events
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            // Linha em branco fecha o evento (sem `data` não há evento)
            let event = self.event.take();
            if self.data.is_empty() {
                return None;
            }
            let data = std::mem::take(&mut self.data).join("\n");
            return Some(SseEvent {
                event: event.unwrap_or_else(|| "message".to_string()),
                data,
            });
        }
        if line.starts_with(':') {
            return None; // keepalive
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => self.data.push(value.to_string()),
            _ => {} // id, retry: sem uso aqui
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, data: &str) -> SseEvent {
        SseEvent {
            event: name.into(),
            data: data.into(),
        }
    }

    #[test]
    fn reads_events_split_across_chunks_and_skips_keepalives() {
        let stream = b"event: attempt\ndata: {\"attemptId\":\"a1\",\"index\":0}\n\n: keepalive\n\nevent: delta\ndata: {\"text\":\"SEL\"}\n\nevent: done\ndata: {}\n\n";
        for size in [1, 3, 7, stream.len()] {
            let mut reader = SseReader::default();
            let events: Vec<SseEvent> = stream.chunks(size).flat_map(|chunk| reader.feed(chunk)).collect();
            assert_eq!(
                events,
                [
                    event("attempt", r#"{"attemptId":"a1","index":0}"#),
                    event("delta", r#"{"text":"SEL"}"#),
                    event("done", "{}"),
                ],
                "pedaços de {size}"
            );
        }
    }

    #[test]
    fn handles_crlf_multiline_data_and_events_without_name() {
        let mut reader = SseReader::default();
        let events = reader.feed(b"data: a\r\ndata: b\r\n\r\nevent: usage\r\n\r\n");
        assert_eq!(events, [event("message", "a\nb")]);
        // Evento sem data não sai; o nome não vaza para o próximo
        let events = reader.feed(b"data: c\n\n");
        assert_eq!(events, [event("message", "c")]);
    }
}
