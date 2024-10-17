use crate::client::Client;

pub struct PendingSerial<'a> {
    serial: Option<u64>,
    client: &'a Client,
}

impl<'a> PendingSerial<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self {
            serial: None,
            client,
        }
    }

    pub fn get(&mut self) -> u64 {
        *self.serial.get_or_insert_with(|| self.client.next_serial())
    }
}
